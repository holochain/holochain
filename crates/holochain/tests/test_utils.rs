#![allow(dead_code)]

use anyhow::Result;
use holochain::conductor::ConductorHandle;
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
use holochain::fixt::NamedInvocation;
use holochain::fixt::ZomeCallInvocationFixturator;
use holochain::{
    conductor::api::ZomeCall,
    conductor::api::{AdminRequest, AdminResponse, AppRequest},
};
use holochain_conductor_api::AppResponse;
use holochain_types::prelude::*;
use holochain_util::tokio_helper;
use holochain_wasm_test_utils::TestWasm;
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

pub async fn start_holochain(config_path: PathBuf) -> SupervisedChild {
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
    spawn_output(&mut child);
    check_started(&mut child).await;
    SupervisedChild("Holochain".to_string(), child)
}

pub async fn call_foo_fn(app_port: u16, original_dna_hash: DnaHash) {
    // Connect to App Interface
    let (mut app_tx, _) = websocket_client_by_port(app_port).await.unwrap();
    let cell_id = CellId::from((original_dna_hash, fake_agent_pubkey_1()));
    call_zome_fn(&mut app_tx, cell_id, TestWasm::Foo, "foo".into(), ()).await;
}

pub async fn call_zome_fn<S>(
    app_tx: &mut WebsocketSender,
    cell_id: CellId,
    wasm: TestWasm,
    fn_name: String,
    input: S,
) where
    S: Serialize + std::fmt::Debug,
{
    let call: ZomeCall = ZomeCallInvocationFixturator::new(NamedInvocation(
        cell_id,
        wasm,
        fn_name,
        ExternIO::encode(input).unwrap(),
    ))
    .next()
    .unwrap()
    .into();
    let request = AppRequest::ZomeCallInvocation(Box::new(call));
    let response = app_tx.request(request);
    let call_response = check_timeout(response, 6000).await;
    trace!(?call_response);
    assert_matches!(call_response, AppResponse::ZomeCallInvocation(_));
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
    role_id: AppRoleId,
    timeout: u64,
) -> DnaHash {
    register_and_install_dna_named(
        client,
        orig_dna_hash,
        agent_key,
        dna_path,
        properties,
        role_id,
        "test".to_string(),
        timeout,
    )
    .await
}

pub async fn register_and_install_dna_named(
    client: &mut WebsocketSender,
    orig_dna_hash: DnaHash,
    agent_key: AgentPubKey,
    dna_path: PathBuf,
    properties: Option<YamlProperties>,
    role_id: AppRoleId,
    name: String,
    timeout: u64,
) -> DnaHash {
    let register_payload = RegisterDnaPayload {
        uid: None,
        properties,
        source: DnaSource::Path(dna_path),
    };
    let request = AdminRequest::RegisterDna(Box::new(register_payload));
    let response = client.request(request);
    let response = check_timeout_named("RegisterDna", response, timeout).await;
    assert_matches!(response, AdminResponse::DnaRegistered(_));
    let dna_hash = if let AdminResponse::DnaRegistered(h) = response {
        h.clone()
    } else {
        orig_dna_hash
    };

    let dna_payload = InstallAppDnaPayload {
        hash: dna_hash.clone(),
        role_id,
        membrane_proof: None,
    };
    let payload = InstallAppPayload {
        dnas: vec![dna_payload],
        installed_app_id: name,
        agent_key,
    };
    let request = AdminRequest::InstallApp(Box::new(payload));
    let response = client.request(request);
    let response = check_timeout_named("InstallApp", response, timeout).await;
    assert_matches!(response, AdminResponse::AppInstalled(_));
    dna_hash
}

pub fn spawn_output(holochain: &mut Child) {
    let stdout = holochain.stdout.take();
    let stderr = holochain.stderr.take();
    tokio::task::spawn(async move {
        if let Some(stdout) = stdout {
            let mut reader = BufReader::new(stdout).lines();
            while let Ok(Some(line)) = reader.next_line().await {
                trace!("holochain bin stdout: {}", line);
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
        keystore: KeystoreConfig::DangerTestKeystoreLegacyDeprecated,
        db_sync_level: DbSyncLevel::default(),
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
