#![allow(dead_code)]

use ::fixt::*;
use anyhow::Result;
use assert_cmd::prelude::*;
use ed25519_dalek::{Signer, SigningKey};
use futures::Future;
use hdk::prelude::*;
use holochain::conductor::ConductorHandle;
use holochain::{
    conductor::api::ZomeCallParamsSigned,
    conductor::api::{AdminRequest, AdminResponse, AppRequest},
};
use holochain_conductor_api::conductor::paths::DataRootPath;
use holochain_conductor_api::conductor::ConductorConfig;
use holochain_conductor_api::conductor::KeystoreConfig;
use holochain_conductor_api::AdminInterfaceConfig;
use holochain_conductor_api::AppResponse;
use holochain_conductor_api::FullStateDump;
use holochain_conductor_api::InterfaceDriver;
use holochain_types::prelude::*;
use holochain_types::websocket::AllowedOrigins;
use holochain_util::tokio_helper;
use holochain_websocket::WebsocketSender;
use holochain_websocket::{WebsocketReceiver, WebsocketResult};
use matches::assert_matches;
use serde::Serialize;
use std::time::Duration;
use std::{path::PathBuf, process::Stdio};
use tokio::io::AsyncBufReadExt;
use tokio::io::AsyncWriteExt;
use tokio::io::BufReader;
use tokio::process::Child;
use tokio::process::Command;

pub use holochain::sweettest::websocket_client_by_port;
use mr_bundle::FileSystemBundler;

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
    start_holochain_with_lair(config_path, false).await
}

pub async fn start_holochain_with_lair(
    config_path: PathBuf,
    full_keystore: bool,
) -> (SupervisedChild, tokio::sync::oneshot::Receiver<u16>) {
    tracing::info!("\n\n----\nstarting holochain\n----\n\n");
    let cmd = std::process::Command::cargo_bin("holochain").unwrap();
    let mut cmd = Command::from(cmd);
    let rust_log = std::env::var("RUST_LOG").unwrap_or_else(|_| "warn".to_string());
    cmd.arg("--config-path")
        .arg(config_path)
        .env("RUST_LOG", rust_log)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    if full_keystore {
        cmd.arg("--piped").stdin(Stdio::piped());
    }
    let mut child = cmd.spawn().expect("Failed to spawn holochain");
    if full_keystore {
        // Pass in lair keystore password.
        let mut stdin = child.stdin.take().unwrap();
        stdin.write_all("pass".as_bytes()).await.unwrap();
        stdin.flush().await.unwrap();
    }
    // Wait for admin port output.
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

    let cap_secret = fixt!(CapSecret);

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
    assert_matches!(response, AdminResponse::ZomeCallCapabilityGranted(_));
    Ok(cap_secret)
}

pub async fn call_zome_fn_fallible<I>(
    app_tx: &WebsocketSender,
    cell_id: CellId,
    signing_keypair: &SigningKey,
    cap_secret: CapSecret,
    zome_name: ZomeName,
    fn_name: FunctionName,
    input: &I,
) -> AppResponse
where
    I: Serialize + std::fmt::Debug,
{
    let (nonce, expires_at) = holochain_nonce::fresh_nonce(Timestamp::now()).unwrap();
    let signing_key = AgentPubKey::from_raw_32(signing_keypair.verifying_key().as_bytes().to_vec());
    let zome_call_params = ZomeCallParams {
        cap_secret: Some(cap_secret),
        cell_id: cell_id.clone(),
        zome_name: zome_name.clone(),
        fn_name: fn_name.clone(),
        provenance: signing_key,
        payload: ExternIO::encode(input).unwrap(),
        nonce,
        expires_at,
    };
    let (bytes, bytes_hash) = zome_call_params.serialize_and_hash().unwrap();
    let signature = signing_keypair.sign(&bytes_hash);
    let request = AppRequest::CallZome(Box::new(ZomeCallParamsSigned::new(
        bytes,
        Signature::from(signature.to_bytes()),
    )));
    let response = app_tx.request(request);
    check_timeout(response, 6000).await.unwrap()
}

pub async fn call_zome_fn<I>(
    app_tx: &WebsocketSender,
    cell_id: CellId,
    signing_keypair: &SigningKey,
    cap_secret: CapSecret,
    zome_name: ZomeName,
    fn_name: FunctionName,
    input: &I,
) -> ExternIO
where
    I: Serialize + std::fmt::Debug,
{
    let call_response = call_zome_fn_fallible(
        app_tx,
        cell_id,
        signing_keypair,
        cap_secret,
        zome_name,
        fn_name,
        input,
    )
    .await;
    match call_response {
        AppResponse::ZomeCalled(response) => *response,
        _ => panic!("zome call failed {call_response:?}"),
    }
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

pub async fn retry_websocket_client_by_port(
    port: u16,
    mut attempts: usize,
    delay: Duration,
) -> WebsocketResult<(WebsocketSender, WebsocketReceiver)> {
    loop {
        match websocket_client_by_port(port).await {
            Ok(c) => return Ok(c),
            Err(e) => {
                attempts -= 1;
                if attempts == 0 {
                    return Err(e);
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

    let dna_bundle1 = FileSystemBundler::load_from::<ValidatedDnaManifest>(&dna_path)
        .await
        .map(DnaBundle::from)
        .unwrap();
    let dna_bundle = dna_bundle1.clone();
    let (dna, _) = dna_bundle1
        .into_dna_file(mods.clone().serialized().unwrap())
        .await
        .unwrap();
    let dna_hash = dna.dna_hash().clone();

    let resource_id = dna_path.file_name().unwrap().to_str().unwrap().to_string();
    let roles = vec![AppRoleManifest {
        name: role_name,
        dna: AppRoleDnaManifest {
            path: Some(resource_id.clone()),
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

    let resources = vec![(resource_id, dna_bundle)];

    let bundle = AppBundle::new(manifest.clone().into(), resources)
        .unwrap()
        .pack()
        .expect("failed to encode AppBundle to bytes");

    let payload = InstallAppPayload {
        agent_key: None,
        source: AppBundleSource::Bytes(bundle),
        installed_app_id: Some(name),
        network_seed: None,
        roles_settings: Default::default(),
        ignore_genesis_failure: false,
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
    let stdout = holochain.stdout.take().unwrap();
    let stderr = holochain.stderr.take().unwrap();
    let (tx, rx) = tokio::sync::oneshot::channel();
    // Wrap in an Option because it is used in a loop and cannot be cloned.
    let mut tx = Some(tx);
    tokio::task::spawn(async move {
        let mut reader = BufReader::new(stdout).lines();
        while let Ok(Some(line)) = reader.next_line().await {
            println!("holochain bin stdout: {}", &line);
            if let Some(port) = check_line_for_admin_port(&line) {
                if let Some(tx) = tx.take() {
                    let _ = tx.send(port);
                }
            }
        }
    });
    tokio::task::spawn(async move {
        let mut reader = BufReader::new(stderr).lines();
        while let Ok(Some(line)) = reader.next_line().await {
            eprintln!("holochain bin stderr: {}", &line);
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

/// Create test config with test keystore.
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
        ..Default::default()
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
