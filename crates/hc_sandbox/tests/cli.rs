use holochain_cli_sandbox::cli::LaunchInfo;
use holochain_cli_sandbox::config::read_config;
use holochain_conductor_api::conductor::ConductorConfig;
#[cfg(feature = "unstable-dpki")]
use holochain_conductor_api::conductor::DpkiConfig;
use holochain_conductor_api::{AdminRequest, AdminResponse, AppAuthenticationRequest, AppRequest};
use holochain_conductor_api::{AppResponse, CellInfo};
use holochain_types::app::InstalledAppId;
use holochain_types::prelude::{SerializedBytes, SerializedBytesError, Timestamp, YamlProperties};
use holochain_websocket::{
    self as ws, ConnectRequest, WebsocketConfig, WebsocketReceiver, WebsocketResult,
    WebsocketSender,
};
use std::future::Future;
use std::net::ToSocketAddrs;
use std::path::PathBuf;
use std::process::Stdio;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;
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

/// Generates a new sandbox with roles settings overridden by a yaml file passed via
/// the --roles-settings argument and verifies that the modifiers have been set
/// correctly
#[tokio::test(flavor = "multi_thread")]
async fn generate_sandbox_with_roles_settings_override() {
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
            assert!(roles.len() == 3);

            //- Test that the modifiers for role 1 and role 2 have been overridden properly
            let role1 = roles
                .clone()
                .into_iter()
                .find(|r| r.name == "role-1")
                .expect("role1 not found in the manifest of the isntalled app.");

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
            assert_eq!(
                role1.dna.modifiers.origin_time.unwrap(),
                Timestamp::from_micros(1731436443698324)
            );
            assert_eq!(role1.dna.modifiers.quantum_time, None);

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
            assert_eq!(
                role2.dna.modifiers.origin_time.unwrap(),
                Timestamp::from_micros(1731436443698326)
            );
            assert_eq!(
                role2.dna.modifiers.quantum_time.unwrap(),
                std::time::Duration::from_secs(60),
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
            assert_eq!(
                role3.dna.modifiers.origin_time.unwrap(),
                Timestamp::from_micros(1000000000000000)
            );
            assert_eq!(role3.dna.modifiers.quantum_time, None,);
        }
        _ => panic!("AppResponse is of the wrong type"),
    }

    shutdown_sandbox(hc_admin).await;
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

    shutdown_sandbox(sandbox_process).await;
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

    shutdown_sandbox(sandbox_process).await;
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

    shutdown_sandbox(sandbox_process).await;
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

    shutdown_sandbox(sandbox_process).await;
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

    shutdown_sandbox(sandbox_process).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn authorize_zome_call_credentials() {
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
        .write(b"test-phrase\n")
        .await
        .unwrap();

    let exit_code = child.wait().await.unwrap();
    assert!(exit_code.success());

    assert!(PathBuf::from(".hc_auth").exists());
}

#[tokio::test(flavor = "multi_thread")]
async fn call_zome_function() {
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
        .write(b"test-phrase\n")
        .await
        .unwrap();

    let exit_code = child.wait().await.unwrap();
    assert!(exit_code.success(), "Failed with exit code {:?}", exit_code);

    println!("Auth done, ready to call");

    // Make the call
    let mut cmd = get_sandbox_command();
    let mut child = cmd
        .arg("zome-call")
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
        .write(b"test-phrase\n")
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

            // On windows, this task stays alive and holds the stdout pipe open
            // so that the tests don't finish.
            #[cfg(not(windows))]
            tokio::task::spawn(async move {
                while let Ok(Some(line)) = lines.next_line().await {
                    println!("@@@-{line}-@@@");
                }
            });

            return config;
        }
    }
    panic!("getting created conductor config failed");
}

async fn shutdown_sandbox(mut child: Child) {
    #[cfg(unix)]
    {
        use nix::sys::signal::Signal;
        use nix::unistd::Pid;

        let pid = child.id().expect("Failed to get PID");
        nix::sys::signal::kill(Pid::from_raw(pid as i32), Signal::SIGINT)
            .expect("Failed to send SIGINT");

        let exit_code = child.wait().await.unwrap();
        assert!(exit_code.success());
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
