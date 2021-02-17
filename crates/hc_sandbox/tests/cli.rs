use std::future::Future;
use std::sync::Arc;

// TODO: do the create tests first
// TODO: run hc-admin then see if we can call the app websocket
// TODO: Put holochain on the path
// TODO:

use assert_cmd::prelude::*;
use holochain_conductor_api::AppRequest;
use holochain_conductor_api::AppResponse;
use holochain_websocket::{websocket_connect, WebsocketConfig, WebsocketReceiver, WebsocketSender};
use matches::assert_matches;
use portpicker::pick_unused_port;
use tokio::process::Command;
use url2::url2;

const WEBSOCKET_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(3);

async fn websocket_client_by_port(
    port: u16,
) -> anyhow::Result<(WebsocketSender, WebsocketReceiver)> {
    Ok(websocket_connect(
        url2!("ws://127.0.0.1:{}", port),
        Arc::new(WebsocketConfig::default()),
    )
    .await?)
}

async fn call_app_interface(port: u16) {
    tracing::debug!(calling_app_interface = ?port);
    let (mut app_tx, _) = websocket_client_by_port(port)
        .await
        .expect(&format!("Failed to get port {}", port));
    let request = AppRequest::AppInfo {
        installed_app_id: "Stub".to_string(),
    };
    let response = app_tx.request(request);
    let r: AppResponse = check_timeout(response).await;
    assert_matches!(r, AppResponse::AppInfo(None));
}

async fn check_timeout<T>(response: impl Future<Output = Result<T, std::io::Error>>) -> T {
    match tokio::time::timeout(WEBSOCKET_TIMEOUT, response).await {
        Ok(response) => response.expect("Calling websocket failed"),
        Err(_) => {
            panic!("Timed out on request after {:?}", WEBSOCKET_TIMEOUT);
        }
    }
}

/// Runs holochain and creates a temp directory
#[tokio::test(threaded_scheduler)]
#[ignore = "Figure out how to get holochain bin in CI"]
async fn run_holochain() {
    observability::test_run().ok();
    let port: u16 = pick_unused_port().expect("No ports free");
    let cmd = std::process::Command::cargo_bin("hc").unwrap();
    let mut cmd = Command::from(cmd);
    cmd.arg("run")
        .arg(format!("-p={}", port))
        .arg("../../../elemental-chat/elemental-chat.dna")
        .kill_on_drop(true);
    let _hc_admin = cmd.spawn().expect("Failed to spawn holochain");
    tokio::time::delay_for(std::time::Duration::from_secs(4)).await;
    // - Make a call to list app info to the port
    call_app_interface(port).await;
}

#[tokio::test(threaded_scheduler)]
#[ignore = "Figure out how to get holochain bin in CI"]
async fn run_multiple_on_same_port() {
    observability::test_run().ok();
    let port: u16 = pick_unused_port().expect("No ports free");
    let app_port: u16 = pick_unused_port().expect("No ports free");
    let cmd = std::process::Command::cargo_bin("hc").unwrap();
    let mut cmd = Command::from(cmd);
    cmd.arg(format!("-f={}", port))
        .arg("run")
        .arg(format!("-p={}", app_port))
        .arg("../../../elemental-chat/elemental-chat.dna")
        .kill_on_drop(true);
    let _hc_admin = cmd.spawn().expect("Failed to spawn holochain");
    tokio::time::delay_for(std::time::Duration::from_secs(4)).await;
    // - Make a call to list app info to the port
    call_app_interface(app_port).await;

    let cmd = std::process::Command::cargo_bin("hc").unwrap();
    let mut cmd = Command::from(cmd);
    cmd.arg(format!("-f={}", port))
        .arg("call")
        .arg("list-dnas")
        .kill_on_drop(true);
    let _hc_admin2 = cmd.spawn().expect("Failed to spawn holochain");
    tokio::time::delay_for(std::time::Duration::from_secs(4)).await;
}
