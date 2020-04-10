use anyhow::Result;
use assert_cmd::prelude::*;
use holochain_2020::conductor::{
    api::{AdminRequest, AdminResponse},
    config::*,
    error::ConductorError,
    Conductor,
};
use holochain_websocket::*;
use std::sync::Arc;
use std::{
    io::Read,
    path::PathBuf,
    process::{Child, Command, ExitStatus, Stdio},
    time::Duration,
};
use sx_types::observability;
use tempdir::TempDir;
use tokio::stream::StreamExt;
use tracing::*;
use url2::prelude::*;

fn check_started(started: Result<Option<ExitStatus>>, holochain: &mut Child) {
    if let Ok(Some(status)) = started {
        let mut stdout = String::new();
        let mut stderr = String::new();
        if let Some(ref mut so) = holochain.stdout {
            so.read_to_string(&mut stdout).ok();
        }
        if let Some(ref mut se) = holochain.stderr {
            se.read_to_string(&mut stderr).ok();
        }
        panic!(
            "Holochain failed to start. status: {:?}, stdout: {}, stderr: {}",
            status, stdout, stderr
        );
    }
}

fn create_config(mut path: PathBuf, port: u16) -> Result<PathBuf> {
    let config = ConductorConfig {
        admin_interfaces: Some(vec![AdminInterfaceConfig {
            driver: InterfaceDriver::Websocket { port },
        }]),
        ..Default::default()
    };
    path.push("conductor_config.toml");
    std::fs::write(path.clone(), toml::to_string(&config)?)?;
    Ok(path)
}

async fn websocket_client(port: u16) -> Result<(WebsocketSender, WebsocketReceiver)> {
    Ok(websocket_connect(
        url2!("ws://127.0.0.1:{}", port),
        Arc::new(WebsocketConfig::default()),
    )
    .await?)
}

#[tokio::test]
async fn call_admin() -> Result<()> {
    // FIXME: make it possible to bind to port 0
    let port = 9000;

    let tmp_dir = TempDir::new("conductor_cfg")?;
    let config_path = create_config(tmp_dir.into_path(), port)?;

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

    let (mut client, _) = websocket_client(port).await?;
    let request = AdminRequest::AddDna;
    let response = client.request(request).await?;
    assert!(matches!(response, AdminResponse::DnaAdded));

    holochain.kill().expect("Failed to kill holochain");
    Ok(())
}

#[tokio::test]
async fn conductor_admin_interface_runs_from_config() -> Result<()> {
    // FIXME: make it possible to bind to port 0
    let config = ConductorConfig {
        admin_interfaces: Some(vec![AdminInterfaceConfig {
            driver: InterfaceDriver::Websocket { port: 9001 },
        }]),
        ..Default::default()
    };
    let conductor_handle = Conductor::build().with_config(config).await?;
    let (mut client, _) = websocket_client(9001).await?;
    let response = client.request(AdminRequest::AddDna).await;
    // TODO: update to proper response once implemented
    assert!(matches!(
        response,
        Ok(AdminResponse::Unimplemented(AdminRequest::AddDna))
    ));
    conductor_handle.shutdown().await;

    Ok(())
}

// TODO: this test hangs because of client websocket connections not being
// closed on shutdown
#[tokio::test]
#[ignore]
async fn conductor_admin_interface_ends_with_shutdown() -> Result<()> {
    observability::test_run().ok();

    info!("creating config");
    // FIXME: make it possible to bind to port 0
    let config = ConductorConfig {
        admin_interfaces: Some(vec![AdminInterfaceConfig {
            driver: InterfaceDriver::Websocket { port: 9010 },
        }]),
        ..Default::default()
    };
    let conductor_handle = Conductor::build().with_config(config).await?;
    info!("creating config");
    let (mut client, rx): (WebsocketSender, WebsocketReceiver) = websocket_connect(
        url2!("ws://127.0.0.1:{}", 9010),
        Arc::new(WebsocketConfig {
            default_request_timeout_s: 1,
            ..Default::default()
        }),
    )
    .await?;

    // clone handle here so we can still illicitly use it later
    conductor_handle.clone().shutdown().await;

    assert!(matches!(
        conductor_handle.check_running().await,
        Err(ConductorError::ShuttingDown)
    ));

    let incoming: Vec<_> = rx.collect().await;
    assert_eq!(incoming.len(), 0);

    println!("About to make failing request");
    info!("About to make failing request");

    // send a request after the conductor has shutdown
    let response: Result<Result<AdminResponse, _>, tokio::time::Elapsed> =
        tokio::time::timeout(Duration::from_secs(1), client.request(AdminRequest::AddDna)).await;

    // request should have errored since the conductor shut down
    assert!(matches!(response, Ok(Err(_))));

    Ok(())
}
