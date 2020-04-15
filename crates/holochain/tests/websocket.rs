use anyhow::Result;
use assert_cmd::prelude::*;
use futures::Future;
use holochain_2020::conductor::{
    api::{AdminRequest, AdminResponse},
    config::*,
    error::ConductorError,
    Conductor, ConductorHandle,
};
use holochain_websocket::*;
use matches::assert_matches;
use std::sync::Arc;
use std::{
    io::Read,
    path::PathBuf,
    process::{Child, Command, ExitStatus, Stdio},
    time::Duration,
};
use sx_types::{
    observability,
    prelude::*,
    test_utils::{fake_dna, fake_dna_file},
};
use tempdir::TempDir;
use tokio::stream::StreamExt;
use tracing::*;
use url2::prelude::*;
use uuid::Uuid;

fn read_output(holochain: &mut Child) -> String {
    let mut stdout = String::new();
    let mut stderr = String::new();
    if let Some(ref mut so) = holochain.stdout {
        so.read_to_string(&mut stdout).ok();
    }
    if let Some(ref mut se) = holochain.stderr {
        se.read_to_string(&mut stderr).ok();
    }
    format!("stdout: {}, stderr: {}", stdout, stderr)
}

fn check_started(started: Result<Option<ExitStatus>>, holochain: &mut Child) {
    if let Ok(Some(status)) = started {
        let output = read_output(holochain);
        panic!(
            "Holochain failed to start. status: {:?}, {}",
            status, output
        );
    }
}

fn create_config(mut path: PathBuf, port: u16) -> PathBuf {
    let config = ConductorConfig {
        admin_interfaces: Some(vec![AdminInterfaceConfig {
            driver: InterfaceDriver::Websocket { port },
        }]),
        ..Default::default()
    };
    path.push("conductor_config.toml");
    std::fs::write(path.clone(), toml::to_string(&config).unwrap()).unwrap();
    path
}

async fn check_timeout(
    holochain: &mut Child,
    response: impl Future<Output = Result<AdminResponse, std::io::Error>>,
    timeout_millis: u64,
) -> AdminResponse {
    match tokio::time::timeout(std::time::Duration::from_millis(timeout_millis), response).await {
        Ok(response) => response.unwrap(),
        Err(_) => {
            holochain.kill().unwrap();
            let output = read_output(holochain);
            panic!("Timed out on request: {}", output)
        }
    }
}

async fn admin_port(conductor: &ConductorHandle) -> u16 {
    conductor
        .read()
        .await
        .get_arbitrary_admin_websocket_port()
        .expect("No admin port open on conductor")
}

async fn websocket_client(
    conductor: &ConductorHandle,
) -> Result<(WebsocketSender, WebsocketReceiver)> {
    let port = admin_port(conductor).await;
    websocket_client_by_port(port).await
}

async fn websocket_client_by_port(port: u16) -> Result<(WebsocketSender, WebsocketReceiver)> {
    Ok(websocket_connect(
        url2!("ws://127.0.0.1:{}", port),
        Arc::new(WebsocketConfig::default()),
    )
    .await?)
}

#[tokio::test]
async fn call_admin() {
    // TODO: can we make this port 0 and find out the dynamic port later?
    let port = 9909;

    let tmp_dir = TempDir::new("conductor_cfg").unwrap();
    let config_path = create_config(tmp_dir.into_path(), port);

    let mut cmd = Command::cargo_bin("holochain-2020").unwrap();
    cmd.arg("--structured");
    cmd.arg("--config-path");
    cmd.arg(config_path);
    cmd.env("RUST_LOG", "debug");
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());
    let mut holochain = cmd.spawn().expect("Failed to spawn holochain");
    std::thread::sleep(std::time::Duration::from_secs(1));
    let started = holochain.try_wait();
    check_started(started.map_err(Into::into), &mut holochain);

    let (mut client, _) = websocket_client_by_port(port).await.unwrap();

    let uuid = Uuid::new_v4();
    let dna = fake_dna(&uuid.to_string());
    let dna_address = dna.address();

    // Install Dna
    let (fake_dna_path, _tmpdir) = fake_dna_file(dna).unwrap();
    let request = AdminRequest::InstallDna(fake_dna_path, None);
    let response = client.request(request);
    let response = check_timeout(&mut holochain, response, 1000).await;
    assert_matches!(response, AdminResponse::DnaInstalled);

    // List Dnas
    let request = AdminRequest::ListDnas;
    let response = client.request(request);
    let response = check_timeout(&mut holochain, response, 1000).await;
    let expects = vec![dna_address];
    assert_matches!(response, AdminResponse::ListDnas(a) if a == expects);

    holochain.kill().expect("Failed to kill holochain");
}

#[tokio::test]
async fn conductor_admin_interface_runs_from_config() -> Result<()> {
    let config = ConductorConfig {
        admin_interfaces: Some(vec![AdminInterfaceConfig {
            driver: InterfaceDriver::Websocket { port: 0 },
        }]),
        ..Default::default()
    };
    let conductor_handle = Conductor::build().with_config(config).await?;
    let (mut client, _) = websocket_client(&conductor_handle).await?;

    let (fake_dna_path, _tmpdir) = fake_dna_file(fake_dna("")).unwrap();
    let request = AdminRequest::InstallDna(fake_dna_path, None);
    let response = client.request(request).await;
    // TODO: update to proper response once implemented
    assert!(matches!(
        response,
        Ok(AdminResponse::Unimplemented(AdminRequest::InstallDna(
            _request,
            _
        )))
    ));
    conductor_handle.shutdown().await;

    Ok(())
}

#[tokio::test]
async fn conductor_admin_interface_ends_with_shutdown() -> Result<()> {
    observability::test_run().ok();

    info!("creating config");
    let config = ConductorConfig {
        admin_interfaces: Some(vec![AdminInterfaceConfig {
            driver: InterfaceDriver::Websocket { port: 0 },
        }]),
        ..Default::default()
    };
    let conductor_handle = Conductor::build().with_config(config).await?;
    let port = admin_port(&conductor_handle).await;
    info!("building conductor");
    let (mut client, rx): (WebsocketSender, WebsocketReceiver) = websocket_connect(
        url2!("ws://127.0.0.1:{}", port),
        Arc::new(WebsocketConfig {
            default_request_timeout_s: 1,
            ..Default::default()
        }),
    )
    .await?;

    info!("client connect");

    // clone handle here so we can still illicitly use it later
    conductor_handle.clone().shutdown().await;

    info!("shutdown");

    assert!(matches!(
        conductor_handle.check_running().await,
        Err(ConductorError::ShuttingDown)
    ));

    let incoming: Vec<_> = rx.collect().await;
    assert_eq!(incoming.len(), 1);
    assert!(matches!(incoming[0], WebsocketMessage::Close(_)));

    info!("About to make failing request");

    let (fake_dna, _tmpdir) = fake_dna_file(fake_dna("")).unwrap();
    let request = AdminRequest::InstallDna(fake_dna, None);

    // send a request after the conductor has shutdown
    let response: Result<Result<AdminResponse, _>, tokio::time::Elapsed> =
        tokio::time::timeout(Duration::from_secs(1), client.request(request)).await;

    // request should have errored since the conductor shut down,
    // but should not have timed out (which would be an `Err(Err(_))`)
    assert!(matches!(response, Ok(Err(_))));

    Ok(())
}
