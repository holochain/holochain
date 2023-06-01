use assert_cmd::prelude::*;
use holochain_cli_sandbox::cli::LaunchInfo;
use holochain_conductor_api::AppRequest;
use holochain_conductor_api::AppResponse;
use holochain_websocket::{self as ws, WebsocketConfig, WebsocketReceiver, WebsocketSender};
use matches::assert_matches;
use once_cell::sync::Lazy;
use std::future::Future;
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{ChildStdout, Command};
use url2::url2;
use which::which;

const WEBSOCKET_TIMEOUT: Duration = Duration::from_secs(3);

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

async fn new_websocket_client_for_port(
    port: u16,
) -> anyhow::Result<(WebsocketSender, WebsocketReceiver)> {
    Ok(ws::connect(
        url2!("ws://127.0.0.1:{}", port),
        Arc::new(WebsocketConfig::default()),
    )
    .await?)
}

async fn get_app_info(port: u16) {
    tracing::debug!(calling_app_interface = ?port);
    let (mut app_tx, _) = new_websocket_client_for_port(port).await.expect(&format!(
        "Failed to connect to conductor on port [{}]",
        port
    ));
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
        .stderr(Stdio::null())
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
    let mut cmd = get_sandbox_command();
    cmd.env("RUST_BACKTRACE", "1")
        .arg(format!(
            "--holochain-path={}",
            get_holochain_bin_path().to_str().unwrap()
        ))
        .arg("--piped")
        .arg("generate")
        .arg("--in-process-lair")
        .arg("--run=0")
        .arg("tests/fixtures/my-app/")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true);

    let mut hc_admin = cmd.spawn().expect("Failed to spawn holochain");

    let mut child_stdin = hc_admin.stdin.take().unwrap();
    child_stdin.write_all(b"test-phrase\n").await.unwrap();
    drop(child_stdin);

    let mut stdout = hc_admin.stdout.take().unwrap();
    let launch_info = get_launch_info(&mut stdout).await;

    // - Make a call to list app info to the port
    get_app_info(*launch_info.app_ports.first().expect("No app ports found")).await;
}

/// Generates a new sandbox with a single app deployed and tries to list DNA
#[tokio::test(flavor = "multi_thread")]
async fn generate_sandbox_and_call_list_dna() {
    clean_sandboxes().await;
    package_fixture_if_not_packaged().await;

    holochain_trace::test_run().ok();
    let mut cmd = get_sandbox_command();
    cmd.env("RUST_BACKTRACE", "1")
        .arg(format!(
            "--holochain-path={}",
            get_holochain_bin_path().to_str().unwrap()
        ))
        .arg("--piped")
        .arg("generate")
        .arg("--in-process-lair")
        .arg("--run=0")
        .arg("tests/fixtures/my-app/")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true);

    let mut hc_admin = cmd.spawn().expect("Failed to spawn holochain");
    let mut child_stdin = hc_admin.stdin.take().unwrap();
    child_stdin.write_all(b"test-phrase\n").await.unwrap();
    drop(child_stdin);

    let mut stdout = hc_admin.stdout.take().unwrap();
    let launch_info = get_launch_info(&mut stdout).await;

    let mut cmd = get_sandbox_command();
    cmd.env("RUST_BACKTRACE", "1")
        .arg("call")
        .arg(format!("--running={}", launch_info.admin_port))
        .arg("list-dnas")
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::inherit());
    let mut hc_call = cmd.spawn().expect("Failed to spawn holochain");

    let exit_code = hc_call.wait().await.unwrap();
    assert!(exit_code.success());
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

async fn get_launch_info(stdout: &mut ChildStdout) -> LaunchInfo {
    let mut lines = BufReader::new(stdout).lines();
    while let Ok(Some(line)) = lines.next_line().await {
        if let Some(index) = line.find("#!0") {
            let launch_info_str = &line[index + 3..].trim();
            return serde_json::from_str::<LaunchInfo>(launch_info_str).unwrap();
        }
    }

    panic!("Unable to find launch info in sandbox output");
}
