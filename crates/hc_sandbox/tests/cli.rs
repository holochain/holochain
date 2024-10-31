use holochain_cli_sandbox::cli::LaunchInfo;
use holochain_cli_sandbox::config::read_config;
use holochain_conductor_api::conductor::ConductorConfig;
#[cfg(feature = "unstable-dpki")]
use holochain_conductor_api::conductor::DpkiConfig;
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
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, Command};

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
    if PathBuf::from("tests/fixtures/my-app/my-fixture-app.happ").exists()
        && PathBuf::from("tests/fixtures/my-app-deferred/my-fixture-app-deferred.happ").exists()
    {
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
        .stderr(Stdio::inherit())
        .kill_on_drop(true);

    println!("@@ {cmd:?}");

    let hc_admin = input_piped_password(&mut cmd).await;

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
        .stderr(Stdio::inherit())
        .kill_on_drop(true);

    let hc_admin = input_piped_password(&mut cmd).await;

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
        .stderr(Stdio::inherit())
        .kill_on_drop(true);

    let hc_admin = input_piped_password(&mut cmd).await;

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

/// Create a new default sandbox which should have DPKI disabled in the conductor config.
#[cfg(not(feature = "unstable-dpki"))]
#[tokio::test(flavor = "multi_thread")]
async fn default_sandbox_has_dpki_disabled() {
    clean_sandboxes().await;
    package_fixture_if_not_packaged().await;

    holochain_trace::test_run();
    let mut cmd = get_sandbox_command();
    cmd.env("RUST_BACKTRACE", "1")
        .arg("--piped")
        .arg("create")
        .arg("--in-process-lair")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .kill_on_drop(true);

    let mut sandbox_process = input_piped_password(&mut cmd).await;
    let conductor_config = get_created_conductor_config(&mut sandbox_process).await;
    assert!(conductor_config.dpki.no_dpki);
}

/// Create a new default sandbox which should have DPKI enabled in the conductor config.
#[cfg(feature = "unstable-dpki")]
#[tokio::test(flavor = "multi_thread")]
async fn default_sandbox_has_dpki_enabled() {
    clean_sandboxes().await;
    package_fixture_if_not_packaged().await;

    holochain_trace::test_run();
    let mut cmd = get_sandbox_command();
    cmd.env("RUST_BACKTRACE", "1")
        .arg("--piped")
        .arg("create")
        .arg("--in-process-lair")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .kill_on_drop(true);

    let mut sandbox_process = input_piped_password(&mut cmd).await;
    let conductor_config = get_created_conductor_config(&mut sandbox_process).await;
    assert!(!conductor_config.dpki.no_dpki);
}

/// Create a new sandbox with DPKI disabled in the conductor config.
#[cfg(feature = "unstable-dpki")]
#[tokio::test(flavor = "multi_thread")]
async fn create_sandbox_without_dpki() {
    clean_sandboxes().await;
    package_fixture_if_not_packaged().await;

    holochain_trace::test_run();
    let mut cmd = get_sandbox_command();
    cmd.env("RUST_BACKTRACE", "1")
        .arg("--piped")
        .arg("create")
        .arg("--in-process-lair")
        .arg("--no-dpki")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .kill_on_drop(true);

    let mut sandbox_process = input_piped_password(&mut cmd).await;
    let conductor_config = get_created_conductor_config(&mut sandbox_process).await;
    assert!(conductor_config.dpki.no_dpki);
}

/// Create a new default sandbox which should have a test network seed set for DPKI.
#[cfg(feature = "unstable-dpki")]
#[tokio::test(flavor = "multi_thread")]
async fn create_default_sandbox_with_dpki_test_network_seed() {
    clean_sandboxes().await;
    package_fixture_if_not_packaged().await;

    holochain_trace::test_run();
    let mut cmd = get_sandbox_command();
    cmd.env("RUST_BACKTRACE", "1")
        .arg("--piped")
        .arg("create")
        .arg("--in-process-lair")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .kill_on_drop(true);

    let mut sandbox_process = input_piped_password(&mut cmd).await;
    let conductor_config = get_created_conductor_config(&mut sandbox_process).await;
    assert_eq!(
        conductor_config.dpki.network_seed,
        DpkiConfig::testing().network_seed
    );
}

/// Create a new sandbox with a custom DPKI network seed.
#[cfg(feature = "unstable-dpki")]
#[tokio::test(flavor = "multi_thread")]
async fn create_sandbox_with_custom_dpki_network_seed() {
    clean_sandboxes().await;
    package_fixture_if_not_packaged().await;

    holochain_trace::test_run();
    let network_seed = "sandbox_test_dpki";
    let mut cmd = get_sandbox_command();
    cmd.env("RUST_BACKTRACE", "1")
        .arg("--piped")
        .arg("create")
        .arg("--in-process-lair")
        .arg("--dpki-network-seed")
        .arg(network_seed)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .kill_on_drop(true);

    let mut sandbox_process = input_piped_password(&mut cmd).await;
    let conductor_config = get_created_conductor_config(&mut sandbox_process).await;
    assert_eq!(conductor_config.dpki.network_seed, network_seed);
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
    panic!("Unable to find launch info in sandbox output. See stderr above.")
}

async fn input_piped_password(cmd: &mut Command) -> Child {
    let mut child_process = cmd.spawn().expect("Failed to spawn holochain");
    let mut child_stdin = child_process.stdin.take().unwrap();
    child_stdin.write_all(b"test-phrase\n").await.unwrap();
    child_process
}

async fn get_created_conductor_config(process: &mut Child) -> ConductorConfig {
    let stdout = process.stdout.take().unwrap();
    let mut lines = BufReader::new(stdout).lines();
    while let Ok(Some(line)) = lines.next_line().await {
        println!("@@@-{line}-@@@");
        if let Some(index) = line.find("ConfigRootPath") {
            let config_root_path_debug_output = line[index..].trim();
            let config_root_path = config_root_path_debug_output
                .strip_prefix("ConfigRootPath(\"")
                .unwrap()
                .strip_suffix("\")]")
                .unwrap();
            let config = read_config(PathBuf::from_str(config_root_path).unwrap().into())
                .unwrap()
                .unwrap();
            return config;
        }
    }
    panic!("getting created conductor config failed");
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
