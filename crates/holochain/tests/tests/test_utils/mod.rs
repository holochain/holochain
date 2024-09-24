#![allow(dead_code)]

use anyhow::Result;
use arbitrary::Arbitrary;
use ed25519_dalek::{Signer, SigningKey};
use holochain::conductor::ConductorHandle;
use holochain_conductor_api::conductor::paths::DataRootPath;
use holochain_conductor_api::conductor::DpkiConfig;
use holochain_conductor_api::FullStateDump;
use holochain_websocket::WebsocketSender;
use holochain_websocket::{WebsocketReceiver, WebsocketResult};

pub async fn admin_port(conductor: &ConductorHandle) -> u16 {
    conductor
        .get_arbitrary_admin_websocket_port()
        .expect("No admin port open on conductor")
}

pub async fn websocket_client(
    conductor: &ConductorHandle,
) -> Result<(WebsocketSender, WebsocketReceiver)> {
    let port = admin_port(conductor).await;
    Ok(websocket_client_by_port(port).await?)
}

pub use holochain::sweettest::websocket_client_by_port;

use assert_cmd::prelude::*;
use futures::Future;
use holochain_conductor_api::conductor::ConductorConfig;
use holochain_conductor_api::conductor::KeystoreConfig;
use holochain_conductor_api::AdminInterfaceConfig;
use holochain_conductor_api::InterfaceDriver;
use kitsune_p2p_types::config::KitsuneP2pConfig;
use matches::assert_matches;
use serde::Serialize;
use std::time::Duration;
use std::{path::PathBuf, process::Stdio};
use tokio::io::AsyncBufReadExt;
use tokio::io::BufReader;
use tokio::process::Child;
use tokio::process::Command;

use hdk::prelude::*;
use holochain::{
    conductor::api::ZomeCall,
    conductor::api::{AdminRequest, AdminResponse, AppRequest},
};
use holochain_conductor_api::AppResponse;
use holochain_types::prelude::*;
use holochain_types::websocket::AllowedOrigins;
use holochain_util::tokio_helper;

/// Wrapper that synchronously waits for the Child to terminate on drop.
pub struct SupervisedChild(String, Child);

impl Drop for SupervisedChild {
    fn drop(&mut self) {
        tokio_helper::block_forever_on(async move {
            self.1
                .kill()
                .await
                .unwrap_or_else(|_| panic!("Failed to kill {}", self.0));
        });
    }
}

pub async fn start_holochain(
    config_path: PathBuf,
) -> (SupervisedChild, tokio::sync::oneshot::Receiver<u16>) {
    tracing::info!("\n\n----\nstarting holochain\n----\n\n");
    let cmd = std::process::Command::cargo_bin("holochain").unwrap();
    let mut cmd = Command::from(cmd);
    let rust_log = std::env::var("RUST_LOG").unwrap_or_else(|_| "warn".to_string());
    cmd.arg("--structured")
        .arg("--config-path")
        .arg(config_path)
        .env("RUST_LOG", rust_log)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    let mut child = cmd.spawn().expect("Failed to spawn holochain");
    let admin_port = spawn_output(&mut child);
    check_started(&mut child).await;
    (SupervisedChild("Holochain".to_string(), child), admin_port)
}

pub async fn grant_zome_call_capability(
    admin_tx: &mut WebsocketSender,
    cell_id: &CellId,
    zome_name: ZomeName,
    fn_name: FunctionName,
    signing_key: AgentPubKey,
) -> WebsocketResult<CapSecret> {
    let mut fns = BTreeSet::new();
    fns.insert((zome_name, fn_name));
    let functions = GrantedFunctions::Listed(fns);

    let mut assignees = BTreeSet::new();
    assignees.insert(signing_key.clone());

    let mut buf = arbitrary::Unstructured::new(&[]);
    let cap_secret = CapSecret::arbitrary(&mut buf).unwrap();

    let request = AdminRequest::GrantZomeCallCapability(Box::new(GrantZomeCallCapabilityPayload {
        cell_id: cell_id.clone(),
        cap_grant: ZomeCallCapGrant {
            tag: "".into(),
            access: CapAccess::Assigned {
                secret: cap_secret,
                assignees,
            },
            functions,
        },
    }));
    let response = admin_tx.request(request);
    let response = check_timeout(response, 3000).await?;
    assert_matches!(response, AdminResponse::ZomeCallCapabilityGranted);
    Ok(cap_secret)
}

pub async fn call_zome_fn<S>(
    app_tx: &WebsocketSender,
    cell_id: CellId,
    signing_keypair: &SigningKey,
    cap_secret: CapSecret,
    zome_name: ZomeName,
    fn_name: FunctionName,
    input: &S,
) where
    S: Serialize + std::fmt::Debug,
{
    let (nonce, expires_at) = holochain_nonce::fresh_nonce(Timestamp::now()).unwrap();
    let signing_key = AgentPubKey::from_raw_32(signing_keypair.verifying_key().as_bytes().to_vec());
    let zome_call_unsigned = ZomeCallUnsigned {
        cap_secret: Some(cap_secret),
        cell_id: cell_id.clone(),
        zome_name: zome_name.clone(),
        fn_name: fn_name.clone(),
        provenance: signing_key,
        payload: ExternIO::encode(input).unwrap(),
        nonce,
        expires_at,
    };
    let signature = signing_keypair.sign(&zome_call_unsigned.data_to_sign().unwrap());
    let call = ZomeCall {
        cell_id: zome_call_unsigned.cell_id,
        zome_name: zome_call_unsigned.zome_name,
        fn_name: zome_call_unsigned.fn_name,
        payload: zome_call_unsigned.payload,
        cap_secret: zome_call_unsigned.cap_secret,
        provenance: zome_call_unsigned.provenance,
        nonce: zome_call_unsigned.nonce,
        expires_at: zome_call_unsigned.expires_at,
        signature: Signature::from(signature.to_bytes()),
    };
    let request = AppRequest::CallZome(Box::new(call));
    let response = app_tx.request(request);
    let call_response = check_timeout(response, 6000).await.unwrap();
    trace!(?call_response);
    assert_matches!(call_response, AppResponse::ZomeCalled(_));
}

pub async fn attach_app_interface(client: &WebsocketSender, port: Option<u16>) -> u16 {
    let request = AdminRequest::AttachAppInterface {
        port,
        allowed_origins: AllowedOrigins::Any,
        installed_app_id: None,
    };
    let response = client.request(request);
    let response = check_timeout(response, 3000).await.unwrap();
    match response {
        AdminResponse::AppInterfaceAttached { port } => port,
        _ => panic!("Attach app interface failed: {:?}", response),
    }
}

pub async fn retry_admin_interface(
    port: u16,
    mut attempts: usize,
    delay: Duration,
) -> WebsocketSender {
    loop {
        match websocket_client_by_port(port).await {
            Ok(c) => return c.0,
            Err(e) => {
                attempts -= 1;
                if attempts == 0 {
                    panic!("Failed to join admin interface");
                }
                warn!(
                    "Failed with {:?} to open admin interface, trying {} more times",
                    e, attempts
                );
                tokio::time::sleep(delay).await;
            }
        }
    }
}

pub async fn generate_agent_pub_key(
    client: &mut WebsocketSender,
    timeout: u64,
) -> WebsocketResult<AgentPubKey> {
    let request = AdminRequest::GenerateAgentPubKey;
    let response = client
        .request_timeout(request, Duration::from_millis(timeout))
        .await?;

    Ok(unwrap_to::unwrap_to!(response => AdminResponse::AgentPubKeyGenerated).clone())
}

/// Returns the hash of the DNA installed, after modifiers have been applied
pub async fn register_and_install_dna(
    client: &mut WebsocketSender,
    dna_path: PathBuf,
    properties: Option<YamlProperties>,
    role_name: RoleName,
    timeout: u64,
) -> WebsocketResult<CellId> {
    register_and_install_dna_named(
        client,
        dna_path,
        properties,
        role_name,
        "test".to_string(),
        timeout,
    )
    .await
}

/// Returns the hash of the DNA installed, after modifiers have been applied
#[allow(clippy::too_many_arguments)]
pub async fn register_and_install_dna_named(
    client: &mut WebsocketSender,
    dna_path: PathBuf,
    properties: Option<YamlProperties>,
    role_name: RoleName,
    name: String,
    timeout: u64,
) -> WebsocketResult<CellId> {
    let mods = DnaModifiersOpt {
        properties,
        ..Default::default()
    };

    let dna_bundle1 = DnaBundle::read_from_file(&dna_path).await.unwrap();
    let dna_bundle = DnaBundle::read_from_file(&dna_path).await.unwrap();
    let (dna, _) = dna_bundle1
        .into_dna_file(mods.clone().serialized().unwrap())
        .await
        .unwrap();
    let dna_hash = dna.dna_hash().clone();

    let roles = vec![AppRoleManifest {
        name: role_name,
        dna: AppRoleDnaManifest {
            location: Some(DnaLocation::Bundled(dna_path.clone())),
            modifiers: mods,
            installed_hash: None,
            clone_limit: 0,
        },
        provisioning: Some(CellProvisioning::Create { deferred: false }),
    }];

    let manifest = AppManifestCurrentBuilder::default()
        .name(name.clone())
        .description(None)
        .roles(roles)
        .build()
        .unwrap();

    let resources = vec![(dna_path.clone(), dna_bundle)];

    let bundle = AppBundle::new(manifest.clone().into(), resources, dna_path.clone())
        .await
        .unwrap();

    let payload = InstallAppPayload {
        agent_key: None,
        source: AppBundleSource::Bundle(bundle),
        installed_app_id: Some(name),
        network_seed: None,
        membrane_proofs: Default::default(),
        existing_cells: Default::default(),
        ignore_genesis_failure: false,
        allow_throwaway_random_agent_key: true,
    };
    let request = AdminRequest::InstallApp(Box::new(payload));
    let response = client.request(request);
    let response = check_timeout_named("InstallApp", response, timeout).await?;
    if let AdminResponse::AppInstalled(app) = response {
        Ok(CellId::new(dna_hash, app.agent_pub_key))
    } else {
        panic!("InstallApp failed: {:?}", response);
    }
}

pub fn spawn_output(holochain: &mut Child) -> tokio::sync::oneshot::Receiver<u16> {
    let stdout = holochain.stdout.take();
    let stderr = holochain.stderr.take();
    let (tx, rx) = tokio::sync::oneshot::channel();
    let mut tx = Some(tx);
    tokio::task::spawn(async move {
        if let Some(stdout) = stdout {
            let mut reader = BufReader::new(stdout).lines();
            while let Ok(Some(line)) = reader.next_line().await {
                println!("holochain bin stdout: {}", &line);
                tx = tx
                    .take()
                    .and_then(|tx| match check_line_for_admin_port(&line) {
                        Some(port) => {
                            let _ = tx.send(port);
                            None
                        }
                        None => Some(tx),
                    });
            }
        }
    });
    tokio::task::spawn(async move {
        if let Some(stderr) = stderr {
            let mut reader = BufReader::new(stderr).lines();
            while let Ok(Some(line)) = reader.next_line().await {
                eprintln!("holochain bin stderr: {}", &line);
            }
        }
    });
    rx
}

fn check_line_for_admin_port(mut line: &str) -> Option<u16> {
    line = line.strip_prefix("###")?;
    line = line.strip_suffix("###")?;

    let port = line.strip_prefix("ADMIN_PORT:")?;
    port.parse::<u16>().ok()
}

pub async fn check_started(holochain: &mut Child) {
    let started = tokio::time::timeout(std::time::Duration::from_secs(1), holochain.wait()).await;
    if let Ok(status) = started {
        panic!("Holochain failed to start. status: {:?}", status);
    }
}

/// Create test config with test keystore and DPKI disabled
pub fn create_config(port: u16, data_root_path: DataRootPath) -> ConductorConfig {
    ConductorConfig {
        admin_interfaces: Some(vec![AdminInterfaceConfig {
            driver: InterfaceDriver::Websocket {
                port,
                allowed_origins: AllowedOrigins::Any,
            },
        }]),
        data_root_path: Some(data_root_path),
        keystore: KeystoreConfig::DangerTestKeystore,
        dpki: DpkiConfig::disabled(),
        network: KitsuneP2pConfig::testing(),
        ..ConductorConfig::empty()
    }
}

pub fn write_config(mut path: PathBuf, config: &ConductorConfig) -> PathBuf {
    path.push("conductor_config.yml");
    std::fs::write(path.clone(), serde_yaml::to_string(&config).unwrap()).unwrap();
    path
}

#[cfg_attr(feature = "instrument", tracing::instrument(skip(response)))]
pub async fn check_timeout<T>(
    response: impl Future<Output = WebsocketResult<T>>,
    timeout_ms: u64,
) -> WebsocketResult<T> {
    check_timeout_named("<unnamed>", response, timeout_ms).await
}

#[cfg_attr(feature = "instrument", tracing::instrument(skip(response)))]
async fn check_timeout_named<T>(
    name: &'static str,
    response: impl Future<Output = WebsocketResult<T>>,
    timeout_millis: u64,
) -> WebsocketResult<T> {
    match tokio::time::timeout(Duration::from_millis(timeout_millis), response).await {
        Ok(response) => response,
        Err(_) => Err(std::io::Error::other(format!(
            "{}: Timed out on request after {}",
            name, timeout_millis
        ))
        .into()),
    }
}

pub async fn dump_full_state(
    client: &mut WebsocketSender,
    cell_id: CellId,
    dht_ops_cursor: Option<u64>,
) -> WebsocketResult<FullStateDump> {
    let request = AdminRequest::DumpFullState {
        cell_id: Box::new(cell_id),
        dht_ops_cursor,
    };
    let response = client.request(request);
    let response = check_timeout(response, 3000).await?;

    match response {
        AdminResponse::FullStateDumped(state) => Ok(state),
        _ => Err(std::io::Error::other(format!("DumpFullState failed: {:?}", response)).into()),
    }
}
