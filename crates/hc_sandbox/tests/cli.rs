use assert_cmd::prelude::*;
use holochain_conductor_api::AppRequest;
use holochain_conductor_api::AppResponse;
use holochain_websocket::{self as ws, WebsocketConfig, WebsocketReceiver, WebsocketSender};
use matches::assert_matches;
use once_cell::sync::Lazy;
use portpicker::pick_unused_port;
use std::future::Future;
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::Arc;
use tokio::io::AsyncWriteExt;
use tokio::process::Command;
use url2::url2;
use which::which;

const WEBSOCKET_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(3);

static HC_BUILT_PATH: Lazy<PathBuf> = Lazy::new(|| {
    let mut manifest_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest_path.push("../hc/Cargo.toml");

    let out = escargot::CargoBuild::new()
        .bin("hc")
        .current_target()
        .current_release()
        .manifest_path(manifest_path)
        // Not defined on CI
        .target_dir(PathBuf::from(
            option_env!("CARGO_TARGET_DIR").unwrap_or("./target"),
        ))
        .run()
        .unwrap();

    out.path().to_path_buf()
});

static HOLOCHAIN_BUILT_PATH: Lazy<PathBuf> = Lazy::new(|| {
    let mut manifest_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest_path.push("../holochain/Cargo.toml");

    let out = escargot::CargoBuild::new()
        .bin("holochain")
        .current_target()
        .current_release()
        .manifest_path(manifest_path)
        .target_dir(PathBuf::from(
            option_env!("CARGO_TARGET_DIR").unwrap_or("./target"),
        ))
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

async fn check_timeout<T>(response: impl Future<Output = Result<T, ws::WebsocketError>>) -> T {
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

    get_hc_command()
        .arg("dna")
        .arg("pack")
        .arg("tests/fixtures/my-app/dna")
        .stdout(Stdio::null())
        .status()
        .await
        .expect("Failed to pack DNA");

    get_hc_command()
        .arg("app")
        .arg("pack")
        .arg("tests/fixtures/my-app")
        .stdout(Stdio::null())
        .status()
        .await
        .expect("Failed to pack hApp");
}

async fn clean_sandboxes() {
    get_sandbox_command()
        .arg("clean")
        .stdout(Stdio::null())
        .status()
        .await
        .unwrap();
}

/// Generates a new sandbox with a single app deployed and tries to get app info
#[tokio::test(flavor = "multi_thread")]
async fn generate_sandbox_and_connect() {
    clean_sandboxes().await;
    package_fixture_if_not_packaged().await;

    holochain_trace::test_run().ok();
    let port: u16 = pick_unused_port().expect("No ports free");
    let mut cmd = get_sandbox_command();
    cmd.env("RUST_BACKTRACE", "1")
        .arg(format!(
            "--holochain-path={}",
            get_holochain_bin_path().to_str().unwrap()
        ))
        .arg("--piped")
        .arg("generate")
        .arg("--in-process-lair")
        .arg(format!("--run={}", port))
        .arg("tests/fixtures/my-app/")
        .stdin(Stdio::piped())
        //.stdout(Stdio::null())
        .kill_on_drop(true);

    let hc_admin = cmd.spawn().expect("Failed to spawn holochain");

    let mut child_stdin = hc_admin.stdin.unwrap();
    child_stdin.write_all(b"test-phrase\n").await.unwrap();
    drop(child_stdin);

    tokio::time::sleep(std::time::Duration::from_secs(5)).await;
    // - Make a call to list app info to the port
    call_app_interface(port).await;
}

/// Generates a new sandbox with a single app deployed and tries to list DNA
#[tokio::test(flavor = "multi_thread")]
async fn generate_sandbox_and_call_list_dna() {
    clean_sandboxes().await;
    package_fixture_if_not_packaged().await;

    holochain_trace::test_run().ok();
    let port: u16 = pick_unused_port().expect("No ports free");
    let app_port: u16 = pick_unused_port().expect("No ports free");
    let mut cmd = get_sandbox_command();
    cmd.env("RUST_BACKTRACE", "1")
        .arg(format!("-f={}", port))
        .arg(format!(
            "--holochain-path={}",
            get_holochain_bin_path().to_str().unwrap()
        ))
        .arg("--piped")
        .arg("generate")
        .arg("--in-process-lair")
        .arg(format!("--run={}", app_port))
        .arg("tests/fixtures/my-app/")
        .stdin(Stdio::piped())
        //.stdout(Stdio::null())
        .kill_on_drop(true);

    let hc_admin = cmd.spawn().expect("Failed to spawn holochain");
    let mut child_stdin = hc_admin.stdin.unwrap();
    child_stdin.write_all(b"test-phrase\n").await.unwrap();
    drop(child_stdin);

    tokio::time::sleep(std::time::Duration::from_secs(5)).await;
    // - Make a call to list app info to the port
    call_app_interface(app_port).await;

    let mut cmd = get_sandbox_command();
    cmd.env("RUST_BACKTRACE", "1")
        .arg(format!("-f={}", port))
        .arg(format!(
            "--holochain-path={}",
            get_holochain_bin_path().to_str().unwrap()
        ))
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

fn get_hc_command() -> Command {
    Command::new(match which("hc") {
        Ok(p) => p,
        Err(_) => HC_BUILT_PATH.clone(),
    })
}

fn get_holochain_bin_path() -> PathBuf {
    match which("holochain") {
        Ok(p) => p,
        Err(_) => HOLOCHAIN_BUILT_PATH.clone(),
    }
}

fn get_sandbox_command() -> Command {
    match which("hc-sandbox") {
        Ok(p) => Command::new(p),
        Err(_) => Command::from(std::process::Command::cargo_bin("hc-sandbox").unwrap()),
    }
}
