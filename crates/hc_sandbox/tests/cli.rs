use holo_hash::{DnaHash, DnaHashB64};
use holochain_cli_sandbox::cli::LaunchInfo;
use holochain_client::{AdminWebsocket, AllowedOrigins};
use holochain_conductor_api::{
    AdminInterfaceConfig, AdminRequest, AdminResponse, AppAuthenticationRequest, AppRequest,
    InterfaceDriver,
};
use holochain_conductor_api::{AppResponse, CellInfo};
use holochain_conductor_config::config::{read_config, write_config};
use holochain_types::app::InstalledAppId;
use holochain_types::prelude::{SerializedBytes, SerializedBytesError, YamlProperties};
use holochain_websocket::{
    self as ws, ConnectRequest, WebsocketConfig, WebsocketReceiver, WebsocketResult,
    WebsocketSender,
};
use kitsune2_api::DynLocalAgent;
use kitsune2_core::Ed25519LocalAgent;
use kitsune2_test_utils::agent::AgentBuilder;
use std::collections::HashSet;
use std::future::Future;
use std::net::ToSocketAddrs;
use std::path::{Path, PathBuf};
use std::process::{ExitStatus, Output, Stdio};
use std::str::from_utf8;
use std::sync::Arc;
use std::time::{Duration, UNIX_EPOCH};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
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

async fn get_app_info(admin_port: u16, installed_app_id: InstalledAppId, port: u16) -> AppResponse {
    tracing::debug!(calling_app_interface = ?port, admin = ?admin_port);

    let (admin_tx, _admin_rx) = new_websocket_client_for_port::<AdminResponse>(admin_port)
        .await
        .unwrap_or_else(|_| panic!("Failed to connect to conductor on port [{}]", admin_port));

    let issue_token_response = admin_tx
        .request(AdminRequest::IssueAppAuthenticationToken(
            installed_app_id.clone().into(),
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

    tokio::time::timeout(Duration::from_secs(60), async move {
        let app_response: AppResponse;
        loop {
            let request = AppRequest::AppInfo;
            let response = app_tx.request(request);
            let r: AppResponse = check_timeout(response).await;
            match &r {
                AppResponse::AppInfo(Some(_)) => {
                    app_response = r;
                    break;
                }
                AppResponse::AppInfo(None) => {
                    // The sandbox hasn't installed the app yet
                    tokio::time::sleep(Duration::from_millis(100)).await;
                }
                _ => {
                    panic!("Unexpected response {:?}", r);
                }
            }
        }
        app_response
    })
    .await
    .unwrap_or_else(|_| {
        panic!("Timeout waiting for the sandbox to install the app {installed_app_id}")
    })
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
    static PACKAGE_SEMAPHORE: tokio::sync::Semaphore = tokio::sync::Semaphore::const_new(1);
    let _lock = PACKAGE_SEMAPHORE.acquire().await.unwrap();

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

async fn clean_sandboxes(cur_dir: &Path) {
    let mut cmd = get_sandbox_command();
        cmd.arg("clean")
        .current_dir(&cur_dir);
    println!("@@ Clean: {cmd:?}");
    let status = cmd.status().await.unwrap();
    assert_eq!(status, ExitStatus::default());
    println!("@@ Clean Complete");
}


async fn list_sandboxes(cur_dir: &Path) -> Output {
    let mut cmd = get_sandbox_command();
    cmd.env("RUST_BACKTRACE", "1")
        .arg(format!(
            "--holochain-path={}",
            get_holochain_bin_path().to_str().unwrap()
        ))
        .arg("--piped")
        .arg("list")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .current_dir(&cur_dir)
        .kill_on_drop(true);

    println!("@@ List: {cmd:?}");
    let hc_admin = input_piped_password(&mut cmd).await;
    let output = hc_admin.wait_with_output().await.unwrap();
    assert!(output.status.success());
    println!("@@ List Complete");
    output
}


/// Test "clean" of an empty folder.
#[tokio::test(flavor = "multi_thread")]
async fn clean_empty() {
    let temp_dir = tempfile::TempDir::new().unwrap();
    std::fs::create_dir_all(&temp_dir).unwrap();

    let mut cmd = get_sandbox_command();
    cmd.arg("clean")
        .current_dir(&temp_dir.path());
    let output = cmd.output().await.unwrap();
    assert_eq!(output.status, ExitStatus::default());
    let stdout = String::from_utf8(output.stdout).unwrap();
    let line_count = stdout.lines().count();
    assert_eq!(line_count, 1);
}

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

/// Test "clean" on a bogus .hc file that we don't have permissions.
#[cfg(unix)]
#[tokio::test(flavor = "multi_thread")]
async fn clean_no_permission() {
    let temp_dir = tempfile::TempDir::new().unwrap();
    std::fs::create_dir_all(&temp_dir).unwrap();

    let file_path = temp_dir.path().join(".hc");
    std::fs::write(&file_path, "/tmp/bogus").unwrap();
    std::fs::set_permissions(&file_path, std::fs::Permissions::from_mode(0o000)).unwrap();

    let mut cmd = get_sandbox_command();
    cmd.arg("clean")
        .current_dir(&temp_dir.path());
    let output = cmd.output().await.unwrap();
    assert!(!output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    let line_count = stdout.lines().count();
    assert_eq!(line_count, 0);
}



/// Test "clean" with a ".hc" file containing one bogus path.
#[tokio::test(flavor = "multi_thread")]
async fn clean_one() {
    let temp_dir = tempfile::TempDir::new().unwrap();
    std::fs::create_dir_all(&temp_dir).unwrap();
    let file_path = temp_dir.path().join(".hc");
    std::fs::write(&file_path, "/tmp/bogus").unwrap();

    let mut cmd = get_sandbox_command();
    cmd.arg("clean")
        .current_dir(&temp_dir.path());
    let output = cmd.output().await.unwrap();
    assert_eq!(output.status, ExitStatus::default());
    let stdout = String::from_utf8(output.stdout).unwrap();
    let line_count = stdout.lines().count();
    assert_eq!(line_count, 2);
}

/// Test "remove" in empty folder
#[tokio::test(flavor = "multi_thread")]
async fn remove_empty() {
    let temp_dir = tempfile::TempDir::new().unwrap();
    std::fs::create_dir_all(&temp_dir).unwrap();

    let mut cmd = get_sandbox_command();
    cmd.arg("remove")
        .arg("0")
        .current_dir(&temp_dir.path());
    let output = cmd.output().await.unwrap();
    assert_eq!(output.status, ExitStatus::default());
    let stdout = String::from_utf8(output.stdout).unwrap();
    let line_count = stdout.lines().count();
    assert_eq!(line_count, 2);
}

/// Test "remove" with a ".hc" file containing one bogus path.
#[tokio::test(flavor = "multi_thread")]
async fn remove_one() {
    let temp_dir = tempfile::TempDir::new().unwrap();
    std::fs::create_dir_all(&temp_dir).unwrap();
    let file_path = temp_dir.path().join(".hc");
    std::fs::write(&file_path, "/tmp/bogus").unwrap();

    let mut cmd = get_sandbox_command();
    cmd.arg("remove")
        .arg("0")
        .current_dir(&temp_dir.path());
    let output = cmd.output().await.unwrap();
    assert_eq!(output.status, ExitStatus::default());
    let stdout = String::from_utf8(output.stdout).unwrap();
    let line_count = stdout.lines().count();
    assert_eq!(line_count, 2);
}

/// Test "remove" with a ".hc" file containing one bogus and one real path.
#[tokio::test(flavor = "multi_thread")]
async fn remove_two() {
    let temp_dir = tempfile::TempDir::new().unwrap();
    std::fs::create_dir_all(&temp_dir).unwrap();
    let file_path = temp_dir.path().join(".hc");
    std::fs::write(&file_path, "/tmp/bogus").unwrap();

    package_fixture_if_not_packaged().await;
    holochain_trace::test_run();

    let app_path = std::env::current_dir().unwrap().join("tests/fixtures/my-app/");
    let mut cmd = get_sandbox_command();
    cmd.env("RUST_BACKTRACE", "1")
        .arg(format!(
            "--holochain-path={}",
            get_holochain_bin_path().to_str().unwrap()
        ))
        .arg("--piped")
        .arg("generate")
        .arg("--in-process-lair")
        .arg(app_path)
        .current_dir(temp_dir.path())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .kill_on_drop(true);
    println!("@@ {cmd:?}");
    let hc_admin = input_piped_password(&mut cmd).await;
    let output = hc_admin.wait_with_output().await.unwrap();
    assert_eq!(output.status, ExitStatus::default());

    let mut cmd = get_sandbox_command();
    cmd.arg("remove")
        .arg("1")
        .current_dir(&temp_dir.path());
    println!("@@ Remove: {cmd:?}");
    let output = cmd.output().await.unwrap();
    println!("@@ Remove Complete");
    assert_eq!(output.status, ExitStatus::default());
    let stdout = String::from_utf8(output.stdout).unwrap();
    let line_count = stdout.lines().count();
    assert_eq!(line_count, 1);
}


/// "list" test
/// Runs: list, generate, list, clean, list
#[tokio::test(flavor = "multi_thread")]
async fn list_and_clean() {
    let temp_dir = tempfile::TempDir::new().unwrap();
    std::fs::create_dir_all(&temp_dir).unwrap();

    clean_sandboxes(temp_dir.path()).await;
    package_fixture_if_not_packaged().await;
    holochain_trace::test_run();

    let output = list_sandboxes(temp_dir.path()).await;
    println!("@@ {output:?}");
    let stdout = String::from_utf8(output.stdout).unwrap();
    let line_count1 = stdout.lines().count();

    let app_path = std::env::current_dir().unwrap().join("tests/fixtures/my-app/");


    let mut cmd = get_sandbox_command();
    cmd.env("RUST_BACKTRACE", "1")
        .arg(format!(
            "--holochain-path={}",
            get_holochain_bin_path().to_str().unwrap()
        ))
        .arg("--piped")
        .arg("generate")
        .arg("--in-process-lair")
        .arg(app_path)
        .current_dir(temp_dir.path())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .kill_on_drop(true);
    println!("@@ {cmd:?}");
    let hc_admin = input_piped_password(&mut cmd).await;
    let output = hc_admin.wait_with_output().await.unwrap();
    assert_eq!(output.status, ExitStatus::default());

    let output = list_sandboxes(temp_dir.path()).await;
    println!("@@ {output:?}");
    let stdout = String::from_utf8(output.stdout).unwrap();
    let line_count2 = stdout.lines().count();
    assert!(line_count1 < line_count2);

    clean_sandboxes(temp_dir.path()).await;
    let output = list_sandboxes(temp_dir.path()).await;
    let stdout = String::from_utf8(output.stdout).unwrap();
    let line_count3 = stdout.lines().count();
    assert_eq!(line_count3, line_count1);
}


/// Test "run" with a missing index
#[tokio::test(flavor = "multi_thread")]
async fn run_missing() {
    let temp_dir = tempfile::TempDir::new().unwrap();
    std::fs::create_dir_all(&temp_dir).unwrap();
    let file_path = temp_dir.path().join(".hc");
    std::fs::write(&file_path, "/tmp/bogus").unwrap();

    package_fixture_if_not_packaged().await;
    holochain_trace::test_run();

    let app_path = std::env::current_dir().unwrap().join("tests/fixtures/my-app/");
    let mut cmd = get_sandbox_command();
    cmd.env("RUST_BACKTRACE", "1")
        .arg(format!(
            "--holochain-path={}",
            get_holochain_bin_path().to_str().unwrap()
        ))
        .arg("--piped")
        .arg("generate")
        .arg("--in-process-lair")
        .arg(app_path)
        .current_dir(temp_dir.path())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .kill_on_drop(true);
    println!("@@ {cmd:?}");
    let hc_admin = input_piped_password(&mut cmd).await;
    let output = hc_admin.wait_with_output().await.unwrap();
    assert_eq!(output.status, ExitStatus::default());

    let mut cmd = get_sandbox_command();
    cmd.arg("run")
        .arg("1")
        .current_dir(&temp_dir.path())
        .kill_on_drop(true);
    println!("@@ Run: {cmd:?}");
    let output = cmd.output().await.unwrap();
    println!("@@ Run Complete");
    assert_eq!(output.status, ExitStatus::default());

    let mut cmd = get_sandbox_command();
    cmd.arg("run")
        .arg("0")
        .current_dir(&temp_dir.path())
        .kill_on_drop(true);
    println!("@@ Run: {cmd:?}");
    let output = cmd.output().await.unwrap();
    println!("@@ Run Complete");
    assert!(!output.status.success());
}

/// Generates a new sandbox with a single app deployed and tries to get app info
#[tokio::test(flavor = "multi_thread")]
async fn generate_sandbox_and_connect() {
    clean_sandboxes(&std::env::current_dir().unwrap()).await;
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

    let mut hc_admin = input_piped_password(&mut cmd).await;

    let launch_info = get_launch_info(&mut hc_admin).await;

    // - Connect to the app interface and wait for the app to show up in AppInfo
    get_app_info(
        launch_info.admin_port,
        "test-app".into(),
        *launch_info.app_ports.first().expect("No app ports found"),
    )
    .await;

    shutdown_sandbox(hc_admin).await;
}

/// Generates a new sandbox with a single app deployed and tries to list DNA
#[tokio::test(flavor = "multi_thread")]
async fn generate_sandbox_and_call_list_dna() {
    clean_sandboxes(&std::env::current_dir().unwrap()).await;
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

    let mut hc_admin = input_piped_password(&mut cmd).await;

    let launch_info = get_launch_info(&mut hc_admin).await;

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

    shutdown_sandbox(hc_admin).await;
}

/// Generates a new sandbox with a single app deployed with membrane_proof_deferred
/// set to true and tries to list DNA
#[tokio::test(flavor = "multi_thread")]
async fn generate_sandbox_memproof_deferred_and_call_list_dna() {
    clean_sandboxes(&std::env::current_dir().unwrap()).await;
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

    let mut hc_admin = input_piped_password(&mut cmd).await;

    let launch_info = get_launch_info(&mut hc_admin).await;

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

    shutdown_sandbox(hc_admin).await;
}

/// Generates a new sandbox with a single app deployed and tries to list DNA
/// This tests that the conductor gets started up and connected to propely
/// upon calling `hc-sandbox call`
#[tokio::test(flavor = "multi_thread")]
async fn generate_non_running_sandbox_and_call_list_dna() {
    clean_sandboxes(&std::env::current_dir().unwrap()).await;
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
        .arg("tests/fixtures/my-app/")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .kill_on_drop(true);

    let hc_admin = input_piped_password(&mut cmd).await;
    hc_admin.wait_with_output().await.unwrap();

    let mut cmd = get_sandbox_command();
    cmd.env("RUST_BACKTRACE", "1")
        .arg("--piped")
        .arg(format!(
            "--holochain-path={}",
            get_holochain_bin_path().to_str().unwrap()
        ))
        .arg("call")
        .arg("list-dnas")
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::inherit());

    let mut hc_call = input_piped_password(&mut cmd).await;

    let exit_code = hc_call.wait().await.unwrap();
    assert!(exit_code.success());
}

/// Generates a sandbox and overwrites the conductor config with
/// a specific allowed origin on the admin interface, then calls
/// ListDna with the correct origin
#[tokio::test(flavor = "multi_thread")]
async fn generate_sandbox_and_call_list_dna_with_origin() {
    clean_sandboxes(&std::env::current_dir().unwrap()).await;
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
        .arg("tests/fixtures/my-app/")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .kill_on_drop(true);

    let mut hc_admin = input_piped_password(&mut cmd).await;
    let config_root_path = get_config_root_path(&mut hc_admin).await;
    hc_admin.wait().await.unwrap();

    // Overwrite the allowed_origins field
    let mut config = read_config(config_root_path.clone().into())
        .expect("msg")
        .unwrap();

    match config
        .admin_interfaces
        .as_mut()
        .and_then(|ai| ai.get_mut(0))
    {
        Some(admin_interface) => {
            *admin_interface = AdminInterfaceConfig {
                driver: InterfaceDriver::Websocket {
                    port: admin_interface.driver.port(),
                    allowed_origins: AllowedOrigins::Origins(
                        vec!["test-origin".to_string()].into_iter().collect(),
                    ),
                },
            };
        }
        None => panic!("No admin interface config found in conductor config"),
    }

    write_config(config_root_path.clone().into(), &config).unwrap();

    // Verify that admin call fails without specifying an origin to make sure
    // the conductor config has properly been modified
    let mut cmd = get_sandbox_command();
    cmd.env("RUST_BACKTRACE", "1")
        .arg("--piped")
        .arg(format!(
            "--holochain-path={}",
            get_holochain_bin_path().to_str().unwrap()
        ))
        .arg("call")
        .arg("list-dnas")
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::inherit());

    let mut hc_call = input_piped_password(&mut cmd).await;
    let exit_code = hc_call.wait().await.unwrap();
    assert!(!exit_code.success()); // Should fail

    // Verify that admin call succeeds the correct origin
    let mut cmd = get_sandbox_command();
    cmd.env("RUST_BACKTRACE", "1")
        .arg("--piped")
        .arg(format!(
            "--holochain-path={}",
            get_holochain_bin_path().to_str().unwrap()
        ))
        .arg("call")
        .arg("--origin")
        .arg("test-origin")
        .arg("list-dnas")
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::inherit());

    let mut hc_call = input_piped_password(&mut cmd).await;
    let exit_code = hc_call.wait().await.unwrap();
    assert!(exit_code.success());
}

/// Creates a new sandbox and tries to list apps via `hc-sandbox call`
#[tokio::test(flavor = "multi_thread")]
async fn create_sandbox_and_call_list_apps() {
    let temp_dir = tempfile::TempDir::new().unwrap();
    std::fs::create_dir_all(&temp_dir).unwrap();
    //package_fixture_if_not_packaged().await;

    holochain_trace::test_run();
    let mut cmd = get_sandbox_command();
    cmd.env("RUST_BACKTRACE", "1")
        .arg(format!(
            "--holochain-path={}",
            get_holochain_bin_path().to_str().unwrap()
        ))
        .arg("--piped")
        .arg("create")
        .arg("--in-process-lair")
        .current_dir(&temp_dir.path())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit());

    let mut hc_create = input_piped_password(&mut cmd).await;
    hc_create.wait().await.unwrap();

    let mut cmd = get_sandbox_command();
    cmd.env("RUST_BACKTRACE", "1")
        .arg("--piped")
        .arg(format!(
            "--holochain-path={}",
            get_holochain_bin_path().to_str().unwrap()
        ))
        .arg("call")
        .arg("list-apps")
        .current_dir(&temp_dir.path())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit());

    let mut hc_call = input_piped_password(&mut cmd).await;

    let exit_code = hc_call.wait().await.unwrap();
    assert!(exit_code.success());
}

/// Generates a new sandbox with roles settings overridden by a yaml file passed via
/// the --roles-settings argument and verifies that the modifiers have been set
/// correctly
#[tokio::test(flavor = "multi_thread")]
async fn generate_sandbox_with_roles_settings_override() {
    clean_sandboxes(&std::env::current_dir().unwrap()).await;
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
        .arg("--roles-settings")
        .arg("tests/fixtures/roles-settings.yaml")
        .arg("--in-process-lair")
        .arg("--run=0")
        .arg("tests/fixtures/my-app/")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .kill_on_drop(true);

    println!("@@ {cmd:?}");

    let mut hc_admin = input_piped_password(&mut cmd).await;

    let launch_info = get_launch_info(&mut hc_admin).await;

    // - Make a call to list app info to the port
    let app_info = get_app_info(
        launch_info.admin_port,
        "test-app".into(),
        *launch_info.app_ports.first().expect("No app ports found"),
    )
    .await;

    match app_info {
        AppResponse::AppInfo(Some(info)) => {
            let roles = info.manifest.app_roles();
            assert_eq!(3, roles.len());

            //- Test that the modifiers for role 1 and role 2 have been overridden properly
            let role1 = roles
                .clone()
                .into_iter()
                .find(|r| r.name == "role-1")
                .expect("role1 not found in the manifest of the installed app.");

            assert_eq!(
                role1.dna.modifiers.network_seed.unwrap(),
                String::from("some random network seed")
            );
            assert_eq!(
                role1.dna.modifiers.properties.unwrap(),
                YamlProperties::new(serde_yaml::Value::String(String::from(
                    "some properties in the manifest",
                )))
            );

            let role2 = roles
                .clone()
                .into_iter()
                .find(|r| r.name == "role-2")
                .expect("role2 not found in the manifest of the installed app.");

            assert_eq!(
                role2.dna.modifiers.network_seed.unwrap(),
                String::from("another random network seed")
            );
            assert_eq!(
                role2.dna.modifiers.properties.unwrap(),
                YamlProperties::new(serde_yaml::Value::String(String::from(
                    "some properties in the manifest",
                )))
            );

            //- Test that the modifiers for role 3 have remained unaltered, i.e.
            //  stay as defined in ./fixtures/my-app/happ.yaml
            let role3 = roles
                .into_iter()
                .find(|r| r.name == "role-3")
                .expect("role2 not found in the manifest of the installed app.");

            assert_eq!(
                role3.dna.modifiers.network_seed.unwrap(),
                String::from("should remain untouched by roles settings test")
            );
            assert_eq!(
                role3.dna.modifiers.properties.unwrap(),
                YamlProperties::new(serde_yaml::Value::String(String::from(
                    "should remain untouched by roles settings test",
                )))
            );
        }
        _ => panic!("AppResponse is of the wrong type"),
    }

    shutdown_sandbox(hc_admin).await;
}

/// Generates a new sandbox with a single app deployed and tries to list DNA
/// This tests that the conductor gets started up and connected to propely
/// upon calling `hc-sandbox call`
#[tokio::test(flavor = "multi_thread")]
async fn generate_sandbox_and_add_and_list_agent() {
    let temp_dir = tempfile::TempDir::new().unwrap();
    std::fs::create_dir_all(&temp_dir).unwrap();
    package_fixture_if_not_packaged().await;
    let app_path = std::env::current_dir().unwrap().join("tests/fixtures/my-app/");

    // Helper fn to parse process output for agent pub keys.
    fn get_agent_keys_from_process_output(output: Output) -> Vec<String> {
        let mut agents_output = from_utf8(&output.stdout).unwrap().split("AgentPubKey(");
        // Discard characters before the first agent pub key.
        agents_output.next();
        agents_output
            .map(|pub_key| pub_key.split(")").next().unwrap().to_string())
            .collect::<Vec<_>>()
    }

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
        .arg(app_path)
        .arg("--network-seed")
        .arg(format!("{}", UNIX_EPOCH.elapsed().unwrap().as_millis()))
        .current_dir(temp_dir.path())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .kill_on_drop(true);

    let mut hc_admin = input_piped_password(&mut cmd).await;

    let launch_info = get_launch_info(&mut hc_admin).await;

    let admin_ws = AdminWebsocket::connect(
        format!("localhost:{}", launch_info.admin_port).as_str(),
        None,
    )
    .await
    .unwrap();

    // Get all agent infos.
    let agent_infos = admin_ws.agent_info(None).await.unwrap();
    assert_eq!(agent_infos.len(), 2);

    // List all agents over hc-sandbox CLI.
    let mut cmd = get_sandbox_command();
    cmd.env("RUST_BACKTRACE", "1")
        .arg("--piped")
        .arg("call")
        .arg("list-agents")
        .current_dir(temp_dir.path())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit());

    let hc_call = input_piped_password(&mut cmd).await;

    let output = hc_call.wait_with_output().await.unwrap();
    let agent_keys = get_agent_keys_from_process_output(output);
    assert_eq!(agent_keys.len(), 2);

    // Get DNA hashes
    let mut dna_hashes = admin_ws.list_dnas().await.unwrap();

    // List agents of all DNA hashes over hc-sandbox CLI. Should also be two agents.
    let mut cmd = get_sandbox_command();
    cmd.env("RUST_BACKTRACE", "1")
        .arg("--piped")
        .arg("call")
        .arg("list-agents")
        .arg("--dna")
        .arg(dna_hashes[0].to_string())
        .arg("--dna")
        .arg(dna_hashes[1].to_string())
        .current_dir(temp_dir.path())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit());

    let hc_call = input_piped_password(&mut cmd).await;

    let output = hc_call.wait_with_output().await.unwrap();
    let agent_keys = get_agent_keys_from_process_output(output);
    assert_eq!(agent_keys.len(), 2);

    // Drop one of the two DNA hashes.
    dna_hashes.pop().unwrap();

    // Get agent infos for a specific DNA.
    let agent_infos = admin_ws.agent_info(Some(dna_hashes.clone())).await.unwrap();
    assert_eq!(agent_infos.len(), 1);

    // List agents of a specific DNA hash over hc-sandbox CLI.
    let mut cmd = get_sandbox_command();
    cmd.env("RUST_BACKTRACE", "1")
        .arg("--piped")
        .arg("call")
        .arg("list-agents")
        .arg("--dna")
        .arg(dna_hashes[0].to_string())
        .current_dir(temp_dir.path())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit());

    let hc_call = input_piped_password(&mut cmd).await;

    let output = hc_call.wait_with_output().await.unwrap();
    let agent_keys = get_agent_keys_from_process_output(output);
    assert_eq!(agent_keys.len(), 1);

    let space = kitsune2_api::AgentInfoSigned::decode(
        &kitsune2_core::Ed25519Verifier,
        agent_infos[0].as_bytes(),
    )
    .unwrap()
    .space
    .clone();

    let other_agent = AgentBuilder {
        space: Some(space.clone()),
        ..Default::default()
    }
    .build(Arc::new(Ed25519LocalAgent::default()) as DynLocalAgent)
    .encode()
    .unwrap();

    let agent_infos_to_add = format!("[{}]", other_agent); // add-agents expects a JSON array

    let mut cmd = get_sandbox_command();
    cmd.env("RUST_BACKTRACE", "1")
        .arg("--piped")
        .arg("call")
        .arg("add-agents")
        .arg(agent_infos_to_add)
        .current_dir(temp_dir.path())
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::inherit());

    let mut hc_call = input_piped_password(&mut cmd).await;

    let exit_code = hc_call.wait().await.unwrap();
    assert!(exit_code.success());

    let agent_infos = admin_ws.agent_info(None).await.unwrap();
    assert_eq!(agent_infos.len(), 3);

    shutdown_sandbox(hc_admin).await;
}

/// Tests retrieval of agent meta info via `hc sandbox call agent-meta-info`
#[tokio::test(flavor = "multi_thread")]
async fn generate_sandbox_and_call_agent_meta_info() {
    clean_sandboxes(&std::env::current_dir().unwrap()).await;
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

    let mut hc_generate = input_piped_password(&mut cmd).await;

    let launch_info = get_launch_info(&mut hc_generate).await;

    let app_info = get_app_info(
        launch_info.admin_port,
        "test-app".into(),
        *launch_info.app_ports.first().expect("No app ports found"),
    )
    .await;

    let dna_hashes = match app_info {
        AppResponse::AppInfo(Some(info)) => {
            let cell_ids: Vec<Vec<CellInfo>> = info
                .cell_info
                .into_iter()
                .map(|(_, cell_infos)| cell_infos)
                .collect();
            println!("cell_ids: {:?}", cell_ids);
            let dna_hash_1 = match cell_ids[0].first().unwrap() {
                CellInfo::Provisioned(cell) => cell.cell_id.dna_hash().clone(),
                _ => panic!("Cell not provisioned"),
            };
            let dna_hash_2 = match cell_ids[1].first().unwrap() {
                CellInfo::Provisioned(cell) => cell.cell_id.dna_hash().clone(),
                _ => panic!("Cell not provisioned"),
            };
            let dna_hash_3 = match cell_ids[2].first().unwrap() {
                CellInfo::Provisioned(cell) => cell.cell_id.dna_hash().clone(),
                _ => panic!("Cell not provisioned"),
            };
            // The fixture happ contains 3 times the same dna of which 2 have the same dna hash
            // therefore we need to deduplicate here...
            vec![dna_hash_1, dna_hash_2, dna_hash_3]
                .into_iter()
                .collect::<HashSet<DnaHash>>()
                .into_iter()
                .collect::<Vec<DnaHash>>()
        }
        r => panic!("AppResponse does not contain app info: {:?}", r),
    };

    // Needs to get converted to a String (not DnaHashB64) so that the sorting will
    // match the sorting of the JSON output from the `hc sandbox agent-meta-info` call
    let mut dna_hashes_b64: Vec<String> = dna_hashes
        .into_iter()
        .map(|h| DnaHashB64::from(h).to_string())
        .collect();

    // ...and sort to get a consistent order to compare with output
    dna_hashes_b64.sort();

    // Get agent meta info for all dnas
    let mut cmd = get_sandbox_command();
    cmd.env("RUST_BACKTRACE", "1")
        .arg("--piped")
        .arg(format!(
            "--holochain-path={}",
            get_holochain_bin_path().to_str().unwrap()
        ))
        .arg("call")
        .arg("agent-meta-info")
        .arg("--url")
        .arg("wss://someurl:443")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit());

    let mut hc_call = input_piped_password(&mut cmd).await;

    hc_call.wait().await.unwrap();

    let mut output = String::new();
    hc_call
        .stdout
        .take()
        .unwrap()
        .read_to_string(&mut output)
        .await
        .unwrap();

    let expected_output = format!(
        r#"{{
  "{}": {{}},
  "{}": {{}}
}}
"#,
        dna_hashes_b64[0].clone(),
        dna_hashes_b64[1].clone()
    );

    assert_eq!(output, expected_output);

    // Get agent meta info for a specific dna
    let mut cmd = get_sandbox_command();
    cmd.env("RUST_BACKTRACE", "1")
        .arg("--piped")
        .arg(format!(
            "--holochain-path={}",
            get_holochain_bin_path().to_str().unwrap()
        ))
        .arg("call")
        .arg("agent-meta-info")
        .arg("--url")
        .arg("wss://someurl:443")
        .arg("--dna")
        .arg(dna_hashes_b64[0].clone())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit());

    let mut hc_call = input_piped_password(&mut cmd).await;

    hc_call.wait().await.unwrap();

    let mut output = String::new();
    hc_call
        .stdout
        .take()
        .unwrap()
        .read_to_string(&mut output)
        .await
        .unwrap();

    let expected_output = format!(
        r#"{{
  "{}": {{}}
}}
"#,
        dna_hashes_b64[0].clone(),
    );

    assert_eq!(output, expected_output);

    shutdown_sandbox(hc_generate).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn authorize_zome_call_credentials() {
    clean_sandboxes(&std::env::current_dir().unwrap()).await;
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
        .stderr(Stdio::inherit());

    let mut hc_admin = input_piped_password(&mut cmd).await;
    let launch_info = get_launch_info(&mut hc_admin).await;

    // Wait for the app to be available
    get_app_info(
        launch_info.admin_port,
        "test-app".into(),
        *launch_info.app_ports.first().expect("No app ports found"),
    )
    .await;

    // Generate signing credentials
    let mut cmd = get_sandbox_command();
    let mut child = cmd
        .arg("zome-call-auth")
        .arg("--running")
        .arg(launch_info.admin_port.to_string())
        .arg("--piped")
        .arg("test-app")
        .kill_on_drop(true)
        .stdin(Stdio::piped())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()
        .unwrap();

    child
        .stdin
        .take()
        .unwrap()
        .write_all(b"test-phrase\n")
        .await
        .unwrap();

    let exit_code = child.wait().await.unwrap();
    assert!(exit_code.success());

    assert!(PathBuf::from(".hc_auth").exists());

    shutdown_sandbox(hc_admin).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn call_zome_function() {
    clean_sandboxes(&std::env::current_dir().unwrap()).await;
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

    let mut hc_admin = input_piped_password(&mut cmd).await;
    let launch_info = get_launch_info(&mut hc_admin).await;

    println!("Got launch info: {:?}", launch_info);

    // Wait for the app to be available
    let app_info = get_app_info(
        launch_info.admin_port,
        "test-app".into(),
        *launch_info.app_ports.first().expect("No app ports found"),
    )
    .await;

    let dna_hash = match app_info {
        AppResponse::AppInfo(Some(info)) => {
            match info.cell_info.first().unwrap().1.first().unwrap() {
                CellInfo::Provisioned(cell) => cell.cell_id.dna_hash().clone(),
                _ => panic!("Cell not provisioned"),
            }
        }
        r => panic!("AppResponse does not contain app info: {:?}", r),
    };

    // Generate signing credentials
    let mut cmd = get_sandbox_command();
    let mut child = cmd
        .arg("zome-call-auth")
        .arg("--running")
        .arg(launch_info.admin_port.to_string())
        .arg("--piped")
        .arg("test-app")
        .kill_on_drop(true)
        .stdin(Stdio::piped())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()
        .unwrap();

    child
        .stdin
        .take()
        .unwrap()
        .write_all(b"test-phrase\n")
        .await
        .unwrap();

    let exit_code = child.wait().await.unwrap();
    assert!(exit_code.success(), "Failed with exit code {:?}", exit_code);

    // Make the call
    let mut cmd = get_sandbox_command();
    let mut child = cmd
        .arg("zome-call")
        .arg("--running")
        .arg(launch_info.admin_port.to_string())
        .arg("--piped")
        .arg("test-app")
        .arg(dna_hash.to_string())
        .arg("zome1")
        .arg("foo")
        .arg("null")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .unwrap();

    child
        .stdin
        .take()
        .unwrap()
        .write_all(b"test-phrase\n")
        .await
        .unwrap();

    child.wait().await.unwrap();

    let mut output = String::new();
    child
        .stdout
        .take()
        .unwrap()
        .read_to_string(&mut output)
        .await
        .unwrap();

    assert_eq!(output, "\"foo\"\n");

    shutdown_sandbox(hc_admin).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn zome_function_can_return_hash() {
    clean_sandboxes(&std::env::current_dir().unwrap()).await;
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

    let mut hc_admin = input_piped_password(&mut cmd).await;
    let launch_info = get_launch_info(&mut hc_admin).await;

    println!("Got launch info: {:?}", launch_info);

    // Wait for the app to be available
    let app_info = get_app_info(
        launch_info.admin_port,
        "test-app".into(),
        *launch_info.app_ports.first().expect("No app ports found"),
    )
    .await;

    let dna_hash = match app_info {
        AppResponse::AppInfo(Some(info)) => {
            match info.cell_info.first().unwrap().1.first().unwrap() {
                CellInfo::Provisioned(cell) => cell.cell_id.dna_hash().clone(),
                _ => panic!("Cell not provisioned"),
            }
        }
        r => panic!("AppResponse does not contain app info: {:?}", r),
    };

    // Generate signing credentials
    let mut cmd = get_sandbox_command();
    let mut child = cmd
        .arg("zome-call-auth")
        .arg("--running")
        .arg(launch_info.admin_port.to_string())
        .arg("--piped")
        .arg("test-app")
        .kill_on_drop(true)
        .stdin(Stdio::piped())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()
        .unwrap();

    child
        .stdin
        .take()
        .unwrap()
        .write_all(b"test-phrase\n")
        .await
        .unwrap();

    let exit_code = child.wait().await.unwrap();
    assert!(exit_code.success(), "Failed with exit code {:?}", exit_code);

    // Call function that returns the DNA hash
    let mut cmd = get_sandbox_command();
    let mut child = cmd
        .arg("zome-call")
        .arg("--running")
        .arg(launch_info.admin_port.to_string())
        .arg("--piped")
        .arg("test-app")
        .arg(dna_hash.to_string())
        .arg("zome1")
        .arg("get_dna_hash")
        .arg("null")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .unwrap();

    child
        .stdin
        .take()
        .unwrap()
        .write_all(b"test-phrase\n")
        .await
        .unwrap();

    child.wait().await.unwrap();

    let mut output = String::new();
    child
        .stdout
        .take()
        .unwrap()
        .read_to_string(&mut output)
        .await
        .unwrap();

    // Convert the output string into a DNA hash by splitting the string and parsing the
    // individual bytes.
    let dna_hash_parsed = DnaHash::from_raw_39(
        output
            .split("[")
            .last()
            .unwrap()
            .split("]")
            .next()
            .unwrap()
            .split(",")
            .map(|byte| byte.trim().parse().unwrap())
            .collect::<Vec<u8>>(),
    );

    assert_eq!(dna_hash_parsed, dna_hash);

    shutdown_sandbox(hc_admin).await;
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

async fn get_config_root_path(child: &mut Child) -> PathBuf {
    let stdout = child.stdout.take().unwrap();
    let mut lines = BufReader::new(stdout).lines();

    while let Ok(Some(line)) = lines.next_line().await {
        println!("@@@-{line}-@@@");
        if line.contains("Created [ConfigRootPath(\"") {
            let start = line
                .find("(\"")
                .expect("Unexpected config root path line format.");
            let end = line
                .find("\")]")
                .expect("Unexpected config root path line format.");
            return PathBuf::from(&line[(start + 2)..end]);
        }
    }
    panic!("Unable to find conductor root path in sandbox output. See stderr above.")
}

async fn get_launch_info(child: &mut Child) -> LaunchInfo {
    let stdout = child.stdout.take().unwrap();
    let mut lines = BufReader::new(stdout).lines();

    while let Ok(Some(line)) = lines.next_line().await {
        println!("@@@-{line}-@@@");
        if let Some(index) = line.find("#!0") {
            let launch_info_str = &line[index + 3..].trim();

            // On windows, this task stays alive and holds the stdout pipe open
            // so that the tests don't finish.
            #[cfg(not(windows))]
            tokio::task::spawn(async move {
                while let Ok(Some(line)) = lines.next_line().await {
                    println!("@@@-{line}-@@@");
                }
            });

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

async fn shutdown_sandbox(mut child: Child) {
    #[cfg(unix)]
    {
        use nix::sys::signal::Signal;
        use nix::unistd::Pid;

        let pid = child.id().expect("Failed to get PID");
        nix::sys::signal::kill(Pid::from_raw(pid as i32), Signal::SIGINT)
            .expect("Failed to send SIGINT");

        child.wait().await.unwrap();
    }

    #[cfg(not(unix))]
    {
        // Best effort to shut down for platforms that don't support sending signals in a
        // simple way.
        child.kill().await.unwrap();
    }
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
