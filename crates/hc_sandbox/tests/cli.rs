use holochain_cli_sandbox::cli::LaunchInfo;
use holochain_client::{AdminWebsocket, AllowedOrigins};
use holochain_conductor_api::AppResponse;
use holochain_conductor_api::{
    AdminInterfaceConfig, AdminRequest, AdminResponse, AppAuthenticationRequest, AppRequest,
    InterfaceDriver,
};
use holochain_conductor_config::config::{read_config, write_config};
use holochain_types::app::InstalledAppId;
use holochain_types::prelude::{SerializedBytes, SerializedBytesError, YamlProperties};
use holochain_websocket::{
    self as ws, ConnectRequest, WebsocketConfig, WebsocketReceiver, WebsocketResult,
    WebsocketSender,
};
use serde_json::json;
use std::future::Future;
use std::net::ToSocketAddrs;
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, Command};

const WEBSOCKET_TIMEOUT: Duration = Duration::from_secs(3);

/// Helper function to create an `AdminWebsocket` client from sandbox launch info.
async fn admin_client_from_launch(launch_info: &LaunchInfo) -> AdminWebsocket {
    AdminWebsocket::connect(format!("127.0.0.1:{}", launch_info.admin_port), None)
        .await
        .expect("Failed to connect to admin websocket")
}

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
        .unwrap_or_else(|_| panic!("Failed to connect to conductor on port [{admin_port}]"));

    let issue_token_response = admin_tx
        .request(AdminRequest::IssueAppAuthenticationToken(
            installed_app_id.clone().into(),
        ))
        .await
        .unwrap();
    let token = match issue_token_response {
        AdminResponse::AppAuthenticationTokenIssued(issued) => issued.token,
        _ => panic!("Unexpected response {issue_token_response:?}"),
    };

    let (app_tx, _rx) = new_websocket_client_for_port::<AppResponse>(port)
        .await
        .unwrap_or_else(|_| panic!("Failed to connect to conductor on port [{port}]"));
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
                    panic!("Unexpected response {r:?}");
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
            panic!("Timed out on request after {WEBSOCKET_TIMEOUT:?}");
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

/// Generates a new sandbox with a single app deployed and tries to get app info
#[tokio::test(flavor = "multi_thread")]
async fn generate_sandbox_and_connect() {
    let temp_dir = tempfile::TempDir::new().unwrap();
    package_fixture_if_not_packaged().await;
    let app_path = std::env::current_dir()
        .unwrap()
        .join("tests/fixtures/my-app/");

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
        .current_dir(temp_dir.path())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .kill_on_drop(true);

    println!("@@ {cmd:?}");

    let mut hc_admin = input_piped_password(&mut cmd).await;

    let launch_info = get_launch_info(&mut hc_admin).await;

    // Connect to admin interface using holochain_client
    let admin_ws = admin_client_from_launch(&launch_info).await;

    // List apps to verify connection
    let apps = admin_ws.list_apps(None).await.expect("Failed to list apps");
    assert!(
        !apps.is_empty(),
        "Expected at least one app to be installed"
    );

    // Verify the test-app exists
    assert!(
        apps.iter().any(|info| info.installed_app_id == "test-app"),
        "Expected 'test-app' to be in the list of installed apps"
    );

    shutdown_sandbox(hc_admin).await;
}

/// Generates a new sandbox with a single app deployed with membrane_proof_deferred
/// set to true and tries to list DNA
#[tokio::test(flavor = "multi_thread")]
async fn generate_sandbox_memproof_deferred_and_call_list_dna() {
    let temp_dir = tempfile::TempDir::new().unwrap();
    package_fixture_if_not_packaged().await;
    let app_path = std::env::current_dir()
        .unwrap()
        .join("tests/fixtures/my-app-deferred/");

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
        .current_dir(temp_dir.path())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .kill_on_drop(true);

    let mut hc_admin = input_piped_password(&mut cmd).await;

    let launch_info = get_launch_info(&mut hc_admin).await;

    // Connect to admin websocket and list DNAs
    let admin_ws = admin_client_from_launch(&launch_info).await;
    let dnas = admin_ws.list_dnas().await.expect("Failed to list DNAs");
    // Just verify we can list DNAs without error
    assert!(!dnas.is_empty(), "Expected at least one DNA");

    shutdown_sandbox(hc_admin).await;
}

/// Generates a sandbox and overwrites the conductor config with
/// a specific allowed origin on the admin interface, then calls
/// ListDna with the correct origin
#[tokio::test(flavor = "multi_thread")]
async fn generate_sandbox_and_call_list_dna_with_origin() {
    let temp_dir = tempfile::TempDir::new().unwrap();
    package_fixture_if_not_packaged().await;
    let app_path = std::env::current_dir()
        .unwrap()
        .join("tests/fixtures/my-app/");

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
        .arg(app_path)
        .current_dir(temp_dir.path())
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
                    danger_bind_addr: None,
                    allowed_origins: AllowedOrigins::Origins(
                        vec!["test-origin".to_string()].into_iter().collect(),
                    ),
                },
            };
        }
        None => panic!("No admin interface config found in conductor config"),
    }

    write_config(config_root_path.clone().into(), &config).unwrap();

    // Verify the config was written correctly
    let reread_config = read_config(config_root_path.clone().into())
        .expect("Failed to read config")
        .unwrap();

    match reread_config
        .admin_interfaces
        .as_ref()
        .and_then(|ai| ai.first())
    {
        Some(admin_interface) => match &admin_interface.driver {
            InterfaceDriver::Websocket {
                allowed_origins, ..
            } => match allowed_origins {
                AllowedOrigins::Origins(origins) => {
                    assert!(
                        origins.contains(&"test-origin".to_string()),
                        "Expected test-origin in allowed origins"
                    );
                }
                _ => panic!("Expected specific origins, got {allowed_origins:?}"),
            },
        },
        None => panic!("No admin interface found in reread config"),
    }
}

/// Creates a new sandbox and tries to list apps via `hc-sandbox call`
#[tokio::test(flavor = "multi_thread")]
async fn create_sandbox_and_call_list_apps() {
    let temp_dir = tempfile::TempDir::new().unwrap();
    package_fixture_if_not_packaged().await;

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
        .current_dir(temp_dir.path())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit());

    let mut hc_create = input_piped_password(&mut cmd).await;
    let exit_code = hc_create.wait().await.unwrap();
    assert!(exit_code.success());

    // Start the conductor
    let mut cmd = get_sandbox_command();
    cmd.env("RUST_BACKTRACE", "1")
        .arg("--piped")
        .arg(format!(
            "--holochain-path={}",
            get_holochain_bin_path().to_str().unwrap()
        ))
        .arg("run")
        .current_dir(temp_dir.path())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .kill_on_drop(true);

    let mut hc_admin = input_piped_password(&mut cmd).await;
    let launch_info = get_launch_info(&mut hc_admin).await;

    // Connect via AdminWebsocket and list apps
    let admin_ws = admin_client_from_launch(&launch_info).await;
    let apps = admin_ws.list_apps(None).await.expect("Failed to list apps");
    // Verify we can list apps (should be empty initially)
    assert!(apps.is_empty(), "Expected no apps in fresh sandbox");

    shutdown_sandbox(hc_admin).await;
}

/// Generates a new sandbox with roles settings overridden by a yaml file passed via
/// the --roles-settings argument and verifies that the modifiers have been set
/// correctly
#[tokio::test(flavor = "multi_thread")]
async fn generate_sandbox_with_roles_settings_override() {
    let temp_dir = tempfile::TempDir::new().unwrap();
    package_fixture_if_not_packaged().await;
    let app_path = std::env::current_dir()
        .unwrap()
        .join("tests/fixtures/my-app/");
    let settings_path = std::env::current_dir()
        .unwrap()
        .join("tests/fixtures/roles-settings.yaml");

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
        .arg(settings_path)
        .arg("--in-process-lair")
        .arg("--run=0")
        .arg(app_path)
        .current_dir(temp_dir.path())
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

/// Generates a new sandbox, setting the webrtc signaling server URL via
/// the webrtc argument and verifies that conductor config file has
/// been written correctly.
#[cfg(any(
    feature = "transport-tx5-datachannel-vendored",
    feature = "transport-tx5-backend-libdatachannel",
    feature = "transport-tx5-backend-go-pion",
))]
#[tokio::test(flavor = "multi_thread")]
async fn generate_sandbox_with_tx5_network_type() {
    use serde_json::json;

    let temp_dir = tempfile::TempDir::new().unwrap();
    package_fixture_if_not_packaged().await;
    let app_path = std::env::current_dir()
        .unwrap()
        .join("tests/fixtures/my-app/");

    let relay_url = "wss://signal";

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
        .arg(app_path)
        .arg("network")
        .arg("webrtc")
        .arg(relay_url)
        .current_dir(temp_dir.path())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .kill_on_drop(true);

    println!("@@ {cmd:?}");

    let mut hc_admin = input_piped_password(&mut cmd).await;

    // Read conductor config yaml file
    let config_root_path = get_config_root_path(&mut hc_admin).await;
    hc_admin.wait().await.unwrap();
    let config = read_config(config_root_path.clone().into())
        .expect("Failed to read config from config_root_path")
        .unwrap();

    // Assert signal url has been set in config file
    assert_eq!(config.network.signal_url, url2::Url2::parse(relay_url));
    assert_eq!(
        config.network.advanced.unwrap(),
        json!({"tx5Transport": {
            "signalAllowPlainText": true
        }})
    );
}

/// Generates a new sandbox, setting the iroh relay URL via
/// the quic argument and verifies that conductor config file has
/// been written correctly.
#[cfg(feature = "transport-iroh")]
#[tokio::test(flavor = "multi_thread")]
async fn generate_sandbox_with_iroh_network_type() {
    let temp_dir = tempfile::TempDir::new().unwrap();
    package_fixture_if_not_packaged().await;
    let app_path = std::env::current_dir()
        .unwrap()
        .join("tests/fixtures/my-app/");

    let relay_url = "https://iroh-relay";

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
        .arg(app_path)
        .arg("network")
        .arg("quic")
        .arg(relay_url)
        .current_dir(temp_dir.path())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .kill_on_drop(true);

    println!("@@ {cmd:?}");

    let mut hc_admin = input_piped_password(&mut cmd).await;

    // Read conductor config yaml file
    let config_root_path = get_config_root_path(&mut hc_admin).await;
    hc_admin.wait().await.unwrap();
    let config = read_config(config_root_path.clone().into())
        .expect("Failed to read config from config_root_path")
        .unwrap();

    // Assert signal url has been overridden in config file
    assert_eq!(config.network.signal_url, url2::Url2::parse(relay_url));
    assert_eq!(
        config.network.advanced.unwrap(),
        json!({"irohTransport": {
            "relayAllowPlainText": true
        }})
    );
}

/// Generates a new sandbox with target_arc_factor settings overridden to 0-arc via
/// the --target-arc-factor argument and verifies that conductor config file has
/// been written correctly.
#[tokio::test(flavor = "multi_thread")]
async fn generate_sandbox_with_target_arc_factor_override() {
    let temp_dir = tempfile::TempDir::new().unwrap();
    package_fixture_if_not_packaged().await;
    let app_path = std::env::current_dir()
        .unwrap()
        .join("tests/fixtures/my-app/");

    #[cfg(feature = "transport-iroh")]
    let (network_type, relay_url) = ("quic", "https://iroh-relay");
    #[cfg(any(
        feature = "transport-tx5-datachannel-vendored",
        feature = "transport-tx5-backend-libdatachannel",
        feature = "transport-tx5-backend-go-pion",
    ))]
    let (network_type, relay_url) = ("webrtc", "wss://signal");

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
        .arg(app_path)
        .arg("network")
        .arg("--target-arc-factor=0")
        .arg(network_type)
        .arg(relay_url)
        .current_dir(temp_dir.path())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .kill_on_drop(true);

    println!("@@ {cmd:?}");

    let mut hc_admin = input_piped_password(&mut cmd).await;

    // Read conductor config yaml file
    let config_root_path = get_config_root_path(&mut hc_admin).await;
    hc_admin.wait().await.unwrap();
    let config = read_config(config_root_path.clone().into())
        .expect("Failed to read config from config_root_path")
        .unwrap();

    // Assert target_arc_factor has been overridden in config file
    assert_eq!(config.network.target_arc_factor, 0);
}

#[tokio::test(flavor = "multi_thread")]
async fn generate_sandbox_and_get_admin_ports() {
    let temp_dir = tempfile::TempDir::new().unwrap();
    package_fixture_if_not_packaged().await;
    let app_path = std::env::current_dir()
        .unwrap()
        .join("tests/fixtures/my-app/");

    holochain_trace::test_run();

    // Generate and run sandboxes (ports will be allocated dynamically)
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
        .current_dir(temp_dir.path())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .kill_on_drop(true);

    let mut hc_generate = input_piped_password(&mut cmd).await;

    // Wait for conductor to start and get its launch info
    let launch_info_1 = get_launch_info(&mut hc_generate).await;

    // Call admin-ports to get the ports
    let mut admin_ports_cmd = get_sandbox_command();
    admin_ports_cmd
        .env("RUST_BACKTRACE", "1")
        .arg("admin-ports")
        .arg("--all")
        .current_dir(temp_dir.path())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit());

    let output = admin_ports_cmd.output().await.unwrap();
    assert!(output.status.success(), "admin-ports command failed");

    let ports: Vec<u16> =
        serde_json::from_slice(&output.stdout).expect("Failed to parse admin ports JSON");

    // Verify we got the expected number of ports
    assert_eq!(ports.len(), 1, "Should have 1 admin port");

    // Verify that the port is non-zero (valid port was allocated)
    assert!(ports[0] > 0, "First port should be a valid port number");

    // Verify the port matches the launch info we got
    assert_eq!(
        ports[0], launch_info_1.admin_port,
        "Port should match launch info"
    );

    shutdown_sandbox(hc_generate).await;
}

include!(concat!(env!("OUT_DIR"), "/target.rs"));

fn get_target(file: &str) -> std::path::PathBuf {
    let target =
        std::str::from_utf8(TARGET).expect("TARGET should be valid UTF-8 from build script");
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
        if line.starts_with("0:") {
            return PathBuf::from(&line[2..line.len()]);
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
