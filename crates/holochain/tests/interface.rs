use assert_cmd::prelude::*;
use holochain_2020::conductor::api::{
    AdminRequest, AdminResponse, ConductorRequest, ConductorResponse,
};
use holochain_websocket::*;
use std::sync::Arc;
use std::{
    io::Read,
    process::{Child, Command, ExitStatus, Stdio},
};
use url2::prelude::*;

type StdResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

fn check_started(started: Result<Option<ExitStatus>, std::io::Error>, holochain: &mut Child) {
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
async fn call_admin() -> StdResult {
    let port = 9000;
    let mut cmd = Command::cargo_bin("holochain-2020").unwrap();
    cmd.arg("--admin");
    cmd.arg("--structured");
    cmd.arg("--websocket-example");
    cmd.arg(port.to_string());
    cmd.env("RUST_LOG", "debug");
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());
    let mut holochain = cmd.spawn().expect("Failed to spawn holochain");
    std::thread::sleep(std::time::Duration::from_secs(1));
    let started = holochain.try_wait();
    check_started(started, &mut holochain);

    run_websocket(port).await?;

    holochain.kill().expect("Failed to kill holochain");
    Ok(())
}

async fn run_websocket(port: u16) -> StdResult {
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
    let request = Box::new(AdminRequest::AddDna);
    let response: ConductorResponse = send_socket
        .request(ConductorRequest::AdminRequest { request })
        .await?;
    let r = match response {
        ConductorResponse::AdminResponse { response } => {
            if let AdminResponse::DnaAdded = *response {
                true
            } else {
                false
            }
        }
        _ => false,
    };
    assert!(r);

    //assert_eq!(response, ConductorResponse::AdminResponse{ response: Box::new(AdminResponse::DnaAdded) });

    Ok(())
}
