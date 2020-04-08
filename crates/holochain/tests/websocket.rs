use anyhow::Result;
use assert_cmd::prelude::*;
use holochain_2020::conductor::{Conductor, api::{AdminRequest, AdminResponse}, config::*};
use holochain_websocket::*;
use std::sync::Arc;
use std::{
    io::Read,
    process::{Child, Command, ExitStatus, Stdio},
};
use tempdir::TempDir;
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

#[tokio::test]
#[ignore]
async fn call_admin() -> Result<()> {
    let port = 9000;
    let mut cmd = Command::cargo_bin("holochain-2020").unwrap();
    cmd.arg("--structured");
    cmd.env("RUST_LOG", "debug");
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());
    let mut holochain = cmd.spawn().expect("Failed to spawn holochain");
    std::thread::sleep(std::time::Duration::from_secs(1));
    let started = holochain.try_wait();
    check_started(started.map_err(Into::into), &mut holochain);

    run_websocket(port).await?;

    holochain.kill().expect("Failed to kill holochain");
    Ok(())
}

async fn run_websocket(port: u16) -> Result<()> {
    //let (mut send_socket, mut recv_socket) = websocket_connect(
    let r = websocket_connect(
        url2!("ws://127.0.0.1:{}", port),
        Arc::new(WebsocketConfig::default()),
    )
    .await;
    if let Err(ref e) = r {
        dbg!(e);
    }
    let (mut send_socket, _) = r?;
    let request = AdminRequest::AddDna;
    let response = send_socket.request(request).await?;
    let r = if let AdminResponse::DnaAdded = response {
        true
    } else {
        false
    };
    assert!(r);

    //assert_eq!(response, AppResponse::AdminResponse{ response: Box::new(AdminResponse::DnaAdded) });

    Ok(())
}


#[tokio::test]
#[ignore]
async fn conductor_admin_interface_runs_from_config() -> Result<()> {
    let config = ConductorConfig {
        admin_interfaces: Some(vec![AdminInterfaceConfig { driver: InterfaceDriver::Websocket { port: 0 }}]),
        ..Default::default()
    };
    let conductor = Conductor::build().from_config(config).await?;
    conductor.write().await.kill().await;

    Ok(())
}
