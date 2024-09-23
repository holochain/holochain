use holochain_cli_sandbox::cli::LaunchInfo;
use holochain_conductor_api::AppResponse;
use holochain_conductor_api::{AdminRequest, AdminResponse, AppAuthenticationRequest, AppRequest};
use holochain_types::app::InstalledAppId;
use holochain_types::prelude::{SerializedBytes, SerializedBytesError};
use holochain_websocket::{
    self as ws, ConnectRequest, WebsocketConfig, WebsocketReceiver, WebsocketResult,
    WebsocketSender,
};
use matches::assert_matches;
use std::future::Future;
use std::net::ToSocketAddrs;
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::process::Command;

const WEBSOCKET_TIMEOUT: Duration = Duration::from_secs(3);

async fn new_websocket_client_for_port<D>(port: u16) -> anyhow::Result<(WebsocketSender, WsPoll)>
where
    D: std::fmt::Debug,
    SerializedBytes: TryInto<D, Error = SerializedBytesError>,
{
    println!("Client for address: {:?}", format!("localhost:{port}"));
    let (tx, rx) = ws::connect(
        Arc::new(WebsocketConfig::CLIENT_DEFAULT),
        ConnectRequest::new(
            format!("localhost:{port}")
                .to_socket_addrs()
                .unwrap()
                .next()
                .unwrap(),
        ),
    )
    .await?;

    Ok((tx, WsPoll::new::<D>(rx)))
}

async fn get_app_info(admin_port: u16, installed_app_id: InstalledAppId, port: u16) {
    tracing::debug!(calling_app_interface = ?port, admin = ?admin_port);

    let (admin_tx, _admin_rx) = new_websocket_client_for_port::<AdminResponse>(admin_port)
        .await
        .unwrap_or_else(|_| panic!("Failed to connect to conductor on port [{}]", admin_port));

    let issue_token_response = admin_tx
        .request(AdminRequest::IssueAppAuthenticationToken(
            installed_app_id.into(),
        ))
        .await
        .unwrap();
    let token = match issue_token_response {
        AdminResponse::AppAuthenticationTokenIssued(issued) => issued.token,
        _ => panic!("Unexpected response {:?}", issue_token_response),
    };

    let (app_tx, _rx) = new_websocket_client_for_port::<AppResponse>(port)
        .await
        .unwrap_or_else(|_| panic!("Failed to connect to conductor on port [{}]", port));
    app_tx
        .authenticate(AppAuthenticationRequest { token })
        .await
        .unwrap();

    let request = AppRequest::AppInfo;
    let response = app_tx.request(request);
    let r: AppResponse = check_timeout(response).await;
    assert_matches!(r, AppResponse::AppInfo(Some(_)));
}

async fn check_timeout<T>(response: impl Future<Output = WebsocketResult<T>>) -> T {
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

    println!("@@ Package Fixture");

    let mut cmd = get_hc_command();

    cmd.arg("dna").arg("pack").arg("tests/fixtures/my-app/dna");

    println!("@@ {cmd:?}");

    cmd.status().await.expect("Failed to pack DNA");

    let mut cmd = get_hc_command();

    cmd.arg("app").arg("pack").arg("tests/fixtures/my-app");

    println!("@@ {cmd:?}");

    cmd.status().await.expect("Failed to pack hApp");

    println!("@@ Package Fixture deferred memproofs");

    let mut cmd = get_hc_command();

    cmd.arg("dna")
        .arg("pack")
        .arg("tests/fixtures/my-app-deferred/dna");

    println!("@@ {cmd:?}");

    cmd.status().await.expect("Failed to pack DNA");

    let mut cmd = get_hc_command();

    cmd.arg("app")
        .arg("pack")
        .arg("tests/fixtures/my-app-deferred");

    println!("@@ {cmd:?}");

    cmd.status()
        .await
        .expect("Failed to pack hApp with deferred memproofs");

    println!("@@ Package Fixture Complete");
}

async fn clean_sandboxes() {
    println!("@@ Clean");

    let mut cmd = get_sandbox_command();

    cmd.arg("clean");

    println!("@@ {cmd:?}");

    cmd.status().await.unwrap();

    println!("@@ Clean Complete");
}

/// Generates a new sandbox with a single app deployed and tries to get app info
#[tokio::test(flavor = "multi_thread")]
async fn generate_sandbox_and_connect() {
    clean_sandboxes().await;
    package_fixture_if_not_packaged().await;

    holochain_trace::test_run();
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

    println!("@@ {cmd:?}");

    let mut hc_admin = cmd.spawn().expect("Failed to spawn holochain");

    let mut child_stdin = hc_admin.stdin.take().unwrap();
    child_stdin.write_all(b"test-phrase\n").await.unwrap();
    drop(child_stdin);

    let launch_info = get_launch_info(hc_admin).await;

    // - Make a call to list app info to the port
    get_app_info(
        launch_info.admin_port,
        "test-app".into(),
        *launch_info.app_ports.first().expect("No app ports found"),
    )
    .await;
}

/// Generates a new sandbox with a single app deployed and tries to list DNA
#[tokio::test(flavor = "multi_thread")]
async fn generate_sandbox_and_call_list_dna() {
    clean_sandboxes().await;
    package_fixture_if_not_packaged().await;

    holochain_trace::test_run();
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

    let launch_info = get_launch_info(hc_admin).await;

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

/// Generates a new sandbox with a single app deployed with membrane_proof_deferred
/// set to true and tries to list DNA
#[tokio::test(flavor = "multi_thread")]
async fn generate_sandbox_memproof_deferred_and_call_list_dna() {
    clean_sandboxes().await;
    package_fixture_if_not_packaged().await;

    holochain_trace::test_run();
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
        .arg("tests/fixtures/my-app-deferred/")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true);

    let mut hc_admin = cmd.spawn().expect("Failed to spawn holochain");
    let mut child_stdin = hc_admin.stdin.take().unwrap();
    child_stdin.write_all(b"test-phrase\n").await.unwrap();
    drop(child_stdin);

    let launch_info = get_launch_info(hc_admin).await;

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

include!(concat!(env!("OUT_DIR"), "/target.rs"));

fn get_target(file: &str) -> std::path::PathBuf {
    let target = unsafe { std::ffi::OsString::from_encoded_bytes_unchecked(TARGET.to_vec()) };
    let mut target = std::path::PathBuf::from(target);

    #[cfg(not(windows))]
    target.push(file);

    #[cfg(windows)]
    target.push(format!("{}.exe", file));

    if std::fs::metadata(&target).is_err() {
        panic!("to run integration tests for hc_sandbox, you need to build the workspace so the following file exists: {:?}", &target);
    }
    target
}

fn get_hc_command() -> Command {
    Command::new(get_target("hc"))
}

fn get_holochain_bin_path() -> PathBuf {
    get_target("holochain")
}

fn get_sandbox_command() -> Command {
    Command::new(get_target("hc-sandbox"))
}

async fn get_launch_info(mut child: tokio::process::Child) -> LaunchInfo {
    let stdout = child.stdout.take().unwrap();
    let mut lines = BufReader::new(stdout).lines();
    while let Ok(Some(line)) = lines.next_line().await {
        println!("@@@@@-{line}-@@@@@");
        if let Some(index) = line.find("#!0") {
            let launch_info_str = &line[index + 3..].trim();
            return serde_json::from_str::<LaunchInfo>(launch_info_str).unwrap();
        }
    }

    let mut buf = String::new();
    if let Some(stderr) = child.stderr.take() {
        BufReader::new(stderr)
            .read_to_string(&mut buf)
            .await
            .unwrap();
        eprintln!("{buf}");
    } else {
        panic!("No stderr! Was the process handle dropped?");
    }

    panic!("Unable to find launch info in sandbox output. See stderr above.")
}

struct WsPoll(tokio::task::JoinHandle<()>);
impl Drop for WsPoll {
    fn drop(&mut self) {
        self.0.abort();
    }
}
impl WsPoll {
    fn new<D>(mut rx: WebsocketReceiver) -> Self
    where
        D: std::fmt::Debug,
        SerializedBytes: TryInto<D, Error = SerializedBytesError>,
    {
        WsPoll(tokio::task::spawn(async move {
            while rx.recv::<D>().await.is_ok() {}
            println!("Poller exiting");
        }))
    }
}
