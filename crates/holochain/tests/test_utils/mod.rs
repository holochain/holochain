#![allow(dead_code)]

use anyhow::Result;
use arbitrary::Arbitrary;
use ed25519_dalek::{Keypair, Signer};
use holochain::conductor::ConductorHandle;
use holochain_conductor_api::FullStateDump;
use holochain_websocket::WebsocketReceiver;
use holochain_websocket::WebsocketSender;

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
use matches::assert_matches;
use serde::Serialize;
use std::time::Duration;
use std::{path::PathBuf, process::Stdio};
use tokio::io::AsyncBufReadExt;
use tokio::io::BufReader;
use tokio::process::Child;
use tokio::process::Command;
use tracing::instrument;

use hdk::prelude::*;
use holochain::{
    conductor::api::ZomeCall,
    conductor::api::{AdminRequest, AdminResponse, AppRequest},
};
use holochain_conductor_api::AppResponse;
use holochain_types::prelude::*;
use holochain_util::tokio_helper;
use holochain_websocket::*;

/// Wrapper that synchronously waits for the Child to terminate on drop.
pub struct SupervisedChild(String, Child);

impl Drop for SupervisedChild {
    fn drop(&mut self) {
        tokio_helper::block_forever_on(async move {
            self.1
                .kill()
                .await
                .expect(&format!("Failed to kill {}", self.0));
        });
    }
}

pub async fn start_holochain(
    config_path: PathBuf,
) -> (SupervisedChild, tokio::sync::oneshot::Receiver<u16>) {
    tracing::info!("\n\n----\nstarting holochain\n----\n\n");
    let cmd = std::process::Command::cargo_bin("holochain").unwrap();
    let mut cmd = Command::from(cmd);
    cmd.arg("--structured")
        .arg("--config-path")
        .arg(config_path)
        .env("RUST_LOG", "trace")
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
) -> CapSecret {
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
    let response = check_timeout(response, 3000).await;
    assert_matches!(response, AdminResponse::ZomeCallCapabilityGranted);
    cap_secret
}

pub async fn call_zome_fn<S>(
    app_tx: &mut WebsocketSender,
    cell_id: CellId,
    signing_keypair: &Keypair,
    cap_secret: CapSecret,
    zome_name: ZomeName,
    fn_name: FunctionName,
    input: &S,
) where
    S: Serialize + std::fmt::Debug,
{
    let mut nonce = [0; 32];
    rand::Rng::fill(&mut rand::thread_rng(), &mut nonce);
    let signing_key = AgentPubKey::from_raw_32(signing_keypair.public.as_bytes().to_vec());
    let zome_call_unsigned = ZomeCallUnsigned {
        cap_secret: Some(cap_secret),
        cell_id: cell_id.clone(),
        zome_name: zome_name.clone(),
        fn_name: fn_name.clone(),
        provenance: signing_key,
        payload: ExternIO::encode(input).unwrap(),
        nonce: Nonce256Bits::from(nonce),
        expires_at: Timestamp((Timestamp::now().as_millis() + 6000) * 1000),
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
    let call_response = check_timeout(response, 6000).await;
    trace!(?call_response);
    assert_matches!(call_response, AppResponse::ZomeCalled(_));
}

pub async fn attach_app_interface(client: &mut WebsocketSender, port: Option<u16>) -> u16 {
    let request = AdminRequest::AttachAppInterface { port };
    let response = client.request(request);
    let response = check_timeout(response, 3000).await;
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

pub async fn generate_agent_pubkey(client: &mut WebsocketSender, timeout: u64) -> AgentPubKey {
    let request = AdminRequest::GenerateAgentPubKey;
    let response = client.request(request);
    let response = check_timeout_named("GenerateAgentPubkey", response, timeout).await;

    unwrap_to::unwrap_to!(response => AdminResponse::AgentPubKeyGenerated).clone()
}

pub async fn register_and_install_dna(
    client: &mut WebsocketSender,
    orig_dna_hash: DnaHash,
    agent_key: AgentPubKey,
    dna_path: PathBuf,
    properties: Option<YamlProperties>,
    role_name: RoleName,
    timeout: u64,
) -> DnaHash {
    register_and_install_dna_named(
        client,
        orig_dna_hash,
        agent_key,
        dna_path,
        properties,
        role_name,
        "test".to_string(),
        timeout,
    )
    .await
}

pub async fn register_and_install_dna_named(
    client: &mut WebsocketSender,
    _orig_dna_hash: DnaHash,
    agent_key: AgentPubKey,
    dna_path: PathBuf,
    properties: Option<YamlProperties>,
    role_name: RoleName,
    name: String,
    timeout: u64,
) -> DnaHash {
    let mods = DnaModifiersOpt {
        properties,
        ..Default::default()
    };

    let dna_bundle1 = DnaBundle::read_from_file(&dna_path).await.unwrap();
    let dna_bundle = DnaBundle::read_from_file(&dna_path).await.unwrap();
    let (_dna, dna_hash) = dna_bundle1
        .into_dna_file(mods.clone().serialized().unwrap())
        .await
        .unwrap();

    let version = DnaVersionSpec::from(vec![dna_hash.clone().into()]).into();

    let roles = vec![AppRoleManifest {
        name: role_name,
        dna: AppRoleDnaManifest {
            location: Some(DnaLocation::Bundled(dna_path.clone())),
            modifiers: mods,
            version: Some(version),
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

    let bundle = AppBundle::new(
        manifest.clone().into(),
        resources,
        PathBuf::from(dna_path.clone()),
    )
    .await
    .unwrap();

    let payload = InstallAppPayload {
        agent_key,
        source: AppBundleSource::Bundle(bundle),
        installed_app_id: Some(name),
        network_seed: None,
        membrane_proofs: std::collections::HashMap::new(),
    };
    let request = AdminRequest::InstallApp(Box::new(payload));
    let response = client.request(request);
    let response = check_timeout_named("InstallApp", response, timeout).await;
    assert_matches!(response, AdminResponse::AppInstalled(_));
    dna_hash
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
                trace!("holochain bin stdout: {}", line);
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
                trace!("holochain bin stderr: {}", line);
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

pub fn create_config(port: u16, environment_path: PathBuf) -> ConductorConfig {
    ConductorConfig {
        admin_interfaces: Some(vec![AdminInterfaceConfig {
            driver: InterfaceDriver::Websocket { port },
        }]),
        environment_path: environment_path.into(),
        network: None,
        dpki: None,
        keystore: KeystoreConfig::DangerTestKeystore,
        db_sync_strategy: DbSyncStrategy::default(),
        chc_namespace: None,
    }
}

pub fn write_config(mut path: PathBuf, config: &ConductorConfig) -> PathBuf {
    path.push("conductor_config.yml");
    std::fs::write(path.clone(), serde_yaml::to_string(&config).unwrap()).unwrap();
    path
}

#[instrument(skip(response))]
pub async fn check_timeout<T>(
    response: impl Future<Output = Result<T, WebsocketError>>,
    timeout_ms: u64,
) -> T {
    check_timeout_named("<unnamed>", response, timeout_ms).await
}

#[instrument(skip(response))]
async fn check_timeout_named<T>(
    name: &'static str,
    response: impl Future<Output = Result<T, WebsocketError>>,
    timeout_millis: u64,
) -> T {
    // FIXME(stefan): remove this multiplier once it's faster on self-hosted CI
    let timeout_millis = timeout_millis * 4;
    match tokio::time::timeout(std::time::Duration::from_millis(timeout_millis), response).await {
        Ok(response) => response.unwrap(),
        Err(e) => {
            panic!(
                "{}: Timed out on request after {}: {}",
                name, timeout_millis, e
            );
        }
    }
}

pub async fn dump_full_state(
    client: &mut WebsocketSender,
    cell_id: CellId,
    dht_ops_cursor: Option<u64>,
) -> FullStateDump {
    let request = AdminRequest::DumpFullState {
        cell_id: Box::new(cell_id),
        dht_ops_cursor,
    };
    let response = client.request(request);
    let response = check_timeout(response, 3000).await;

    let full_state = match response {
        AdminResponse::FullStateDumped(state) => state,
        _ => panic!("DumpFullState failed: {:?}", response),
    };

    full_state
}
