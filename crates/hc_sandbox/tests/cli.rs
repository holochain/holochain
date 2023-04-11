use std::future::Future;
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::Arc;
use assert_cmd::prelude::*;
use holochain_conductor_api::AppRequest;
use holochain_conductor_api::AppResponse;
use holochain_websocket::{self as ws, WebsocketConfig, WebsocketReceiver, WebsocketSender};
use matches::assert_matches;
use once_cell::sync::Lazy;
use portpicker::pick_unused_port;
use tokio::io::AsyncWriteExt;
use tokio::process::Command;
use url2::url2;

const WEBSOCKET_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(3);

static HOLOCHAIN_BUILT_PATH: Lazy<PathBuf> = Lazy::new(|| {
    let out = escargot::CargoBuild::new()
        .package("holochain")
        .bin("holochain")
        .current_target()
        .release()
        .manifest_path("../holochain/Cargo.toml")
        .target_dir("../../target")
        .run()
        .unwrap();

    out.path().to_path_buf()
});

async fn websocket_client_by_port(
    port: u16,
) -> anyhow::Result<(WebsocketSender, WebsocketReceiver)> {
    Ok(ws::connect(
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

async fn check_timeout<T>(response: impl Future<Output=Result<T, ws::WebsocketError>>) -> T {
    match tokio::time::timeout(WEBSOCKET_TIMEOUT, response).await {
        Ok(response) => response.expect("Calling websocket failed"),
        Err(_) => {
            panic!("Timed out on request after {:?}", WEBSOCKET_TIMEOUT);
        }
    }
}

async fn package_fixture_if_not_packaged() {
    if PathBuf::from("tests/fixtures/my-app/my-fixture-app.happ").exists() {
        return;
    }

    Command::new("hc")
        .arg("dna")
        .arg("pack")
        .arg("tests/fixtures/my-app/dna")
        .stdout(Stdio::null())
        .status()
        .await
        .expect("Failed to pack DNA");

    Command::new("hc")
        .arg("app")
        .arg("pack")
        .arg("tests/fixtures/my-app")
        .stdout(Stdio::null())
        .status()
        .await
        .expect("Failed to pack hApp");
}

fn clean_sandboxes() {
    std::process::Command::cargo_bin("hc-sandbox")
        .unwrap()
        .arg("clean")
        .stdout(Stdio::null())
        .status()
        .unwrap();
}

/// Runs holochain and creates a temp directory
#[tokio::test(flavor = "multi_thread")]
async fn run_holochain() {
    clean_sandboxes();
    package_fixture_if_not_packaged().await;

    holochain_trace::test_run().ok();
    let port: u16 = pick_unused_port().expect("No ports free");
    let cmd = std::process::Command::cargo_bin("hc-sandbox").unwrap();
    let mut cmd = Command::from(cmd);
    cmd.arg(format!("--holochain-path={}", HOLOCHAIN_BUILT_PATH.to_str().unwrap()))
        .arg("--piped")
        .arg("generate")
        .arg(format!("--run={}", port))
        .arg("tests/fixtures/my-app/")
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .kill_on_drop(true);

    let hc_admin = cmd.spawn().expect("Failed to spawn holochain");

    let mut child_stdin = hc_admin.stdin.unwrap();
    child_stdin.write_all(b"test-phrase\n").await.unwrap();
    drop(child_stdin);

    tokio::time::sleep(std::time::Duration::from_secs(5)).await;
    // - Make a call to list app info to the port
    call_app_interface(port).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn run_multiple_on_same_port() {
    clean_sandboxes();
    package_fixture_if_not_packaged().await;

    holochain_trace::test_run().ok();
    let port: u16 = pick_unused_port().expect("No ports free");
    let app_port: u16 = pick_unused_port().expect("No ports free");
    let cmd = std::process::Command::cargo_bin("hc-sandbox").unwrap();
    let mut cmd = Command::from(cmd);
    cmd.arg(format!("-f={}", port))
        .arg(format!("--holochain-path={}", HOLOCHAIN_BUILT_PATH.to_str().unwrap()))
        .arg("--piped")
        .arg("generate")
        .arg(format!("--run={}", app_port))
        .arg("tests/fixtures/my-app/")
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .kill_on_drop(true);

    let hc_admin = cmd.spawn().expect("Failed to spawn holochain");
    let mut child_stdin = hc_admin.stdin.unwrap();
    child_stdin.write_all(b"test-phrase\n").await.unwrap();
    drop(child_stdin);

    tokio::time::sleep(std::time::Duration::from_secs(5)).await;
    // - Make a call to list app info to the port
    call_app_interface(app_port).await;

    let cmd = std::process::Command::cargo_bin("hc-sandbox").unwrap();
    let mut cmd = Command::from(cmd);
    cmd.arg(format!("-f={}", port))
        .arg(format!("--holochain-path={}", HOLOCHAIN_BUILT_PATH.to_str().unwrap()))
        .arg("--piped")
        .arg("call")
        .arg("list-dnas")
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .kill_on_drop(true);
    let _hc_call = cmd.spawn().expect("Failed to spawn holochain");
    let mut child_stdin = _hc_call.stdin.unwrap();
    child_stdin.write_all(b"test-phrase\n").await.unwrap();
    drop(child_stdin);

    tokio::time::sleep(std::time::Duration::from_secs(4)).await;
}
