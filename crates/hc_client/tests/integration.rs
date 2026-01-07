use std::path::PathBuf;
use std::process::Command as StdCommand;
use std::time::Duration;

use anyhow::{ensure, Result};
use holo_hash::{AgentPubKey, AgentPubKeyB64, DnaHash, DnaHashB64};
use holochain::{sweettest::*, test_utils::inline_zomes::simple_crud_zome};
use holochain_conductor_api::{AdminInterfaceConfig, InterfaceDriver};
use holochain_types::prelude::CellId;
use holochain_types::websocket::AllowedOrigins;
use std::collections::BTreeMap;
use tokio::io::AsyncWriteExt;
use tokio::process::Command as TokioCommand;
use tokio::sync::OnceCell;

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
        panic!("to run integration tests for hc_client, you need to build the workspace so the following file exists: {:?}", &target);
    }
    target
}

fn get_hc_client_command() -> StdCommand {
    StdCommand::new(get_target("hc-client"))
}

fn get_hc_command() -> PathBuf {
    get_target("hc")
}

#[tokio::test(flavor = "multi_thread")]
async fn list_dnas() -> Result<()> {
    let mut conductor = SweetConductor::from_standard_config().await;
    let (dna, _, _) = SweetDnaFile::unique_from_inline_zomes(simple_crud_zome()).await;
    let expected_hash = dna.dna_hash().to_string();

    conductor.setup_app("app", &[dna]).await?;

    let admin_port = conductor
        .get_arbitrary_admin_websocket_port()
        .expect("admin port");

    let output = get_hc_client_command()
        .args(["call", "--port", &admin_port.to_string(), "list-dnas"])
        .output()?;

    assert!(
        output.status.success(),
        "cli exit code: {:?}",
        output.status
    );

    let hashes: Vec<String> = serde_json::from_slice(&output.stdout)?;
    assert_eq!(hashes, vec![expected_hash]);

    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn list_apps() -> Result<()> {
    let mut conductor = SweetConductor::from_standard_config().await;
    let (dna, _, _) = SweetDnaFile::unique_from_inline_zomes(simple_crud_zome()).await;

    conductor.setup_app("app", &[dna]).await?;

    let admin_port = conductor
        .get_arbitrary_admin_websocket_port()
        .expect("admin port");

    let output = get_hc_client_command()
        .args(["call", "--port", &admin_port.to_string(), "list-apps"])
        .output()?;

    assert!(
        output.status.success(),
        "cli exit code: {:?}",
        output.status
    );

    let apps: Vec<serde_json::Value> = serde_json::from_slice(&output.stdout)?;
    assert_eq!(apps.len(), 1);
    assert_eq!(apps[0]["installed_app_id"], serde_json::json!("app"));

    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn list_app_interfaces() -> Result<()> {
    let conductor = SweetConductor::from_standard_config().await;

    let admin_port = conductor
        .get_arbitrary_admin_websocket_port()
        .expect("admin port");

    let add_output = get_hc_client_command()
        .args(["call", "--port", &admin_port.to_string(), "add-app-ws"])
        .output()?;

    assert!(
        add_output.status.success(),
        "add-app-ws exit code: {:?}\nstderr: {}",
        add_output.status,
        String::from_utf8_lossy(&add_output.stderr)
    );

    let output = get_hc_client_command()
        .args(["call", "--port", &admin_port.to_string(), "list-app-ws"])
        .output()?;

    assert!(
        output.status.success(),
        "list-app-ws exit code: {:?}\nstderr: {}",
        output.status,
        String::from_utf8_lossy(&output.stderr)
    );

    let interfaces: Vec<serde_json::Value> = serde_json::from_slice(&output.stdout)?;
    assert!(
        !interfaces.is_empty(),
        "Expected at least one app interface. stdout: {}",
        String::from_utf8_lossy(&output.stdout)
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn new_agent() -> Result<()> {
    let conductor = SweetConductor::from_standard_config().await;

    let admin_port = conductor
        .get_arbitrary_admin_websocket_port()
        .expect("admin port");

    let output = get_hc_client_command()
        .args(["call", "--port", &admin_port.to_string(), "new-agent"])
        .output()?;

    assert!(
        output.status.success(),
        "new-agent exit code: {:?} stderr: {}",
        output.status,
        String::from_utf8_lossy(&output.stderr)
    );

    let agent_key_str: String = serde_json::from_slice(&output.stdout)?;
    let agent_key_b64: AgentPubKeyB64 = agent_key_str.parse()?;
    let agent_key: AgentPubKey = agent_key_b64.into();
    assert!(
        !agent_key.get_raw_39().is_empty(),
        "agent key should not be empty"
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn install_app() -> Result<()> {
    let conductor = SweetConductor::from_standard_config().await;

    let admin_port = conductor
        .get_arbitrary_admin_websocket_port()
        .expect("admin port");

    ensure_fixture_packaged().await?;

    let app_path = fixture_path(["my-app", "my-fixture-app.happ"])?;

    let output = get_hc_client_command()
        .args([
            "call",
            "--port",
            &admin_port.to_string(),
            "install-app",
            "--app-id",
            "fixture-app",
            app_path.to_str().unwrap(),
        ])
        .output()?;

    assert!(
        output.status.success(),
        "install-app exit code: {:?} stderr: {}",
        output.status,
        String::from_utf8_lossy(&output.stderr)
    );

    let app_info: serde_json::Value = serde_json::from_slice(&output.stdout)?;
    assert_eq!(
        app_info["installed_app_id"],
        serde_json::json!("fixture-app")
    );

    // Verify the app is listed by the conductor
    let list_output = get_hc_client_command()
        .args(["call", "--port", &admin_port.to_string(), "list-apps"])
        .output()?;

    assert!(list_output.status.success());
    let apps: Vec<serde_json::Value> = serde_json::from_slice(&list_output.stdout)?;
    assert!(apps
        .iter()
        .any(|app| app["installed_app_id"] == serde_json::json!("fixture-app")));

    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn uninstall_app() -> Result<()> {
    let conductor = SweetConductor::from_standard_config().await;

    let admin_port = conductor
        .get_arbitrary_admin_websocket_port()
        .expect("admin port");

    ensure_fixture_packaged().await?;

    let app_path = fixture_path(["my-app", "my-fixture-app.happ"])?;

    // Install the app first
    let install_output = get_hc_client_command()
        .args([
            "call",
            "--port",
            &admin_port.to_string(),
            "install-app",
            "--app-id",
            "fixture-app",
            app_path.to_str().unwrap(),
        ])
        .output()?;

    assert!(install_output.status.success());

    // Confirm install success via list-apps
    let list_after_install = get_hc_client_command()
        .args(["call", "--port", &admin_port.to_string(), "list-apps"])
        .output()?;
    assert!(list_after_install.status.success());
    let apps_after_install: Vec<serde_json::Value> =
        serde_json::from_slice(&list_after_install.stdout)?;
    assert!(apps_after_install
        .iter()
        .any(|app| app["installed_app_id"] == serde_json::json!("fixture-app")));

    // Now uninstall
    let uninstall_output = get_hc_client_command()
        .args([
            "call",
            "--port",
            &admin_port.to_string(),
            "uninstall-app",
            "fixture-app",
        ])
        .output()?;

    assert!(
        uninstall_output.status.success(),
        "uninstall-app exit code: {:?} stderr: {}",
        uninstall_output.status,
        String::from_utf8_lossy(&uninstall_output.stderr)
    );

    let stdout = String::from_utf8_lossy(&uninstall_output.stdout);
    assert!(stdout.contains("Uninstalled app"));

    // Confirm that the app is no longer listed
    let list_output = get_hc_client_command()
        .args(["call", "--port", &admin_port.to_string(), "list-apps"])
        .output()?;

    assert!(list_output.status.success());
    let apps: Vec<serde_json::Value> = serde_json::from_slice(&list_output.stdout)?;
    assert!(!apps
        .iter()
        .any(|app| app["installed_app_id"] == serde_json::json!("fixture-app")));

    Ok(())
}

fn fixture_root() -> Result<PathBuf> {
    Ok(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures"))
}

fn fixture_path(parts: impl IntoIterator<Item = &'static str>) -> Result<PathBuf> {
    let root = fixture_root()?;
    Ok(parts.into_iter().fold(root, |acc, part| acc.join(part)))
}

async fn ensure_fixture_packaged() -> Result<()> {
    static PACK_ONCE: OnceCell<()> = OnceCell::const_new();
    PACK_ONCE
        .get_or_try_init(|| async {
            if fixture_path(["my-app", "my-fixture-app.happ"])?.exists()
                && fixture_path(["my-app", "dna", "a dna.dna"])?.exists()
            {
                return Ok(());
            }

            package_fixture().await
        })
        .await?;
    Ok(())
}

async fn package_fixture() -> Result<()> {
    let hc_bin = get_hc_command();

    let dna_status = TokioCommand::new(&hc_bin)
        .arg("dna")
        .arg("pack")
        .arg(fixture_path(["my-app", "dna"])?)
        .status()
        .await?;
    ensure!(dna_status.success(), "Failed to pack DNA fixture");

    let happ_status = TokioCommand::new(&hc_bin)
        .arg("app")
        .arg("pack")
        .arg(fixture_path(["my-app"])?)
        .status()
        .await?;
    ensure!(happ_status.success(), "Failed to pack hApp fixture");

    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn list_cells_and_dnas() -> Result<()> {
    let conductor = SweetConductor::from_standard_config().await;

    let admin_port = conductor
        .get_arbitrary_admin_websocket_port()
        .expect("admin port");

    ensure_fixture_packaged().await?;

    let app_path = fixture_path(["my-app", "my-fixture-app.happ"])?;

    // Install the app
    let install_output = get_hc_client_command()
        .args([
            "call",
            "--port",
            &admin_port.to_string(),
            "install-app",
            "--app-id",
            "test-app",
            app_path.to_str().unwrap(),
        ])
        .output()?;

    assert!(
        install_output.status.success(),
        "install-app failed: {:?} stderr: {}",
        install_output.status,
        String::from_utf8_lossy(&install_output.stderr)
    );

    // Test list-dnas
    let list_dnas_output = get_hc_client_command()
        .args(["call", "--port", &admin_port.to_string(), "list-dnas"])
        .output()?;

    assert!(
        list_dnas_output.status.success(),
        "list-dnas failed: {:?} stderr: {}",
        list_dnas_output.status,
        String::from_utf8_lossy(&list_dnas_output.stderr)
    );

    let dna_strings: Vec<String> = serde_json::from_slice(&list_dnas_output.stdout)?;
    // The fixture app has 3 roles using the same DNA file.
    // role-1 and role-2 share the same network seed, so they produce 1 DNA hash.
    // role-3 has a different network seed and properties, producing a 2nd DNA hash.
    assert_eq!(dna_strings.len(), 2, "Expected exactly 2 DNA hashes");

    // Parse each DNA hash string to verify it's a valid DnaHash
    let dnas: Vec<DnaHash> = dna_strings
        .into_iter()
        .map(|s| {
            let hash_b64: DnaHashB64 = s.parse()?;
            Ok::<_, anyhow::Error>(hash_b64.into())
        })
        .collect::<Result<Vec<_>, _>>()?;
    for dna in &dnas {
        assert!(!dna.get_raw_39().is_empty(), "DNA hash should not be empty");
    }

    // Test list-cells
    let list_cells_output = get_hc_client_command()
        .args(["call", "--port", &admin_port.to_string(), "list-cells"])
        .output()?;

    assert!(
        list_cells_output.status.success(),
        "list-cells failed: {:?} stderr: {}",
        list_cells_output.status,
        String::from_utf8_lossy(&list_cells_output.stderr)
    );

    let cell_jsons: Vec<serde_json::Value> = serde_json::from_slice(&list_cells_output.stdout)?;
    // The fixture app has 3 roles, but role-1 and role-2 share the same DNA hash + agent,
    // so they collapse into a single cell. role-3 has different modifiers, producing a 2nd cell.
    assert_eq!(cell_jsons.len(), 2, "Expected exactly 2 unique cells");

    // Verify each cell has the expected structure with dna_hash and agent_pub_key
    for cell_json in &cell_jsons {
        assert!(
            cell_json.get("dna_hash").is_some(),
            "Cell should have dna_hash"
        );
        assert!(
            cell_json.get("agent_pub_key").is_some(),
            "Cell should have agent_pub_key"
        );

        let dna_hash_str = cell_json["dna_hash"]
            .as_str()
            .expect("dna_hash should be string");
        let agent_key_str = cell_json["agent_pub_key"]
            .as_str()
            .expect("agent_pub_key should be string");

        // Parse to actual types to ensure they're valid
        let dna_hash_b64: DnaHashB64 = dna_hash_str.parse()?;
        let dna_hash: DnaHash = dna_hash_b64.into();
        let agent_key_b64: AgentPubKeyB64 = agent_key_str.parse()?;
        let agent_key: AgentPubKey = agent_key_b64.into();

        assert!(
            !dna_hash.get_raw_39().is_empty(),
            "DNA hash should not be empty"
        );
        assert!(
            !agent_key.get_raw_39().is_empty(),
            "Agent key should not be empty"
        );

        // Verify we can construct a valid CellId from the parts
        let _cell_id = CellId::new(dna_hash, agent_key);
    }

    // Verify that at least one of the cells uses one of the DNAs we found earlier
    let cell_dna_hashes: Vec<DnaHash> = cell_jsons
        .iter()
        .map(|cell| {
            let hash_str = cell["dna_hash"].as_str().unwrap();
            let hash_b64: DnaHashB64 = hash_str.parse()?;
            Ok::<_, anyhow::Error>(hash_b64.into())
        })
        .collect::<Result<Vec<_>, _>>()?;
    for cell_dna in &cell_dna_hashes {
        assert!(
            dnas.contains(cell_dna),
            "Cell DNA should be in the list of DNAs"
        );
    }

    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn enable_disable_app() -> Result<()> {
    let conductor = SweetConductor::from_standard_config().await;

    let admin_port = conductor
        .get_arbitrary_admin_websocket_port()
        .expect("admin port");

    ensure_fixture_packaged().await?;

    let app_path = fixture_path(["my-app", "my-fixture-app.happ"])?;

    // Install the app
    let install_output = get_hc_client_command()
        .args([
            "call",
            "--port",
            &admin_port.to_string(),
            "install-app",
            "--app-id",
            "toggle-app",
            app_path.to_str().unwrap(),
        ])
        .output()?;

    assert!(
        install_output.status.success(),
        "install-app failed: {:?} stderr: {}",
        install_output.status,
        String::from_utf8_lossy(&install_output.stderr)
    );

    // Verify the app is initially running (enabled)
    let list_output = get_hc_client_command()
        .args(["call", "--port", &admin_port.to_string(), "list-apps"])
        .output()?;

    assert!(list_output.status.success());
    let apps: Vec<serde_json::Value> = serde_json::from_slice(&list_output.stdout)?;
    let app = apps
        .iter()
        .find(|app| app["installed_app_id"] == serde_json::json!("toggle-app"))
        .expect("App should be in the list");

    // Check if the app is enabled (running is deprecated, enabled is the new status)
    let status_type = app["status"]["type"]
        .as_str()
        .expect("status type should be a string");
    assert_eq!(
        status_type, "enabled",
        "App should be enabled after install, got: {:?}",
        app["status"]
    );

    // Disable the app
    let disable_output = get_hc_client_command()
        .args([
            "call",
            "--port",
            &admin_port.to_string(),
            "disable-app",
            "toggle-app",
        ])
        .output()?;

    assert!(
        disable_output.status.success(),
        "disable-app failed: {:?} stderr: {}",
        disable_output.status,
        String::from_utf8_lossy(&disable_output.stderr)
    );

    let stdout = String::from_utf8_lossy(&disable_output.stdout);
    assert!(stdout.contains("Disabled app"));

    // Verify the app is now disabled
    let list_after_disable = get_hc_client_command()
        .args(["call", "--port", &admin_port.to_string(), "list-apps"])
        .output()?;

    assert!(list_after_disable.status.success());
    let apps_after_disable: Vec<serde_json::Value> =
        serde_json::from_slice(&list_after_disable.stdout)?;
    let app_after_disable = apps_after_disable
        .iter()
        .find(|app| app["installed_app_id"] == serde_json::json!("toggle-app"))
        .expect("App should still be in the list");

    let status_type_disabled = app_after_disable["status"]["type"]
        .as_str()
        .expect("status type should be a string");
    assert_eq!(
        status_type_disabled, "disabled",
        "App should be disabled, got: {:?}",
        app_after_disable["status"]
    );

    // Re-enable the app
    let enable_output = get_hc_client_command()
        .args([
            "call",
            "--port",
            &admin_port.to_string(),
            "enable-app",
            "toggle-app",
        ])
        .output()?;

    assert!(
        enable_output.status.success(),
        "enable-app failed: {:?} stderr: {}",
        enable_output.status,
        String::from_utf8_lossy(&enable_output.stderr)
    );

    let stdout = String::from_utf8_lossy(&enable_output.stdout);
    assert!(stdout.contains("Enabled app"));

    // Verify the app is running again
    let list_after_enable = get_hc_client_command()
        .args(["call", "--port", &admin_port.to_string(), "list-apps"])
        .output()?;

    assert!(list_after_enable.status.success());
    let apps_after_enable: Vec<serde_json::Value> =
        serde_json::from_slice(&list_after_enable.stdout)?;
    let app_after_enable = apps_after_enable
        .iter()
        .find(|app| app["installed_app_id"] == serde_json::json!("toggle-app"))
        .expect("App should still be in the list");

    let status_type_enabled = app_after_enable["status"]["type"]
        .as_str()
        .expect("status type should be a string");
    assert_eq!(
        status_type_enabled, "enabled",
        "App should be enabled again after enable, got: {:?}",
        app_after_enable["status"]
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn list_agents() -> Result<()> {
    // Start two conductors
    let mut conductors = SweetConductorBatch::from_standard_config(2).await;

    ensure_fixture_packaged().await?;

    // Create DNA files for setup
    let (dna, _, _) = SweetDnaFile::unique_from_inline_zomes(simple_crud_zome()).await;

    // Install the same app on both conductors
    let _apps = conductors.setup_app("test-app", &[dna]).await?;

    let admin_port_0 = conductors[0]
        .get_arbitrary_admin_websocket_port()
        .expect("admin port 0");
    let admin_port_1 = conductors[1]
        .get_arbitrary_admin_websocket_port()
        .expect("admin port 1");

    // Wait for agent infos to be published to the peer store.
    // Agent infos are not immediately available after app installation - they need
    // time to be published to the network. Without this wait, the list-agents CLI
    // command will return empty results.
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            let agent_infos_0 = conductors[0].get_agent_infos(None).await.unwrap();
            let agent_infos_1 = conductors[1].get_agent_infos(None).await.unwrap();
            if !agent_infos_0.is_empty() && !agent_infos_1.is_empty() {
                break;
            }
            tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
        }
    })
    .await
    .expect("agent infos didn't make it to the peer store");

    // Test list-agents on conductor 0
    let list_agents_0 = get_hc_client_command()
        .args(["call", "--port", &admin_port_0.to_string(), "list-agents"])
        .output()?;

    assert!(
        list_agents_0.status.success(),
        "list-agents failed: {:?} stderr: {}",
        list_agents_0.status,
        String::from_utf8_lossy(&list_agents_0.stderr)
    );
    let agents_0: Vec<serde_json::Value> = serde_json::from_slice(&list_agents_0.stdout)?;
    assert!(!agents_0.is_empty(), "Conductor 0 should have agent info");

    // Test list-agents on conductor 1
    let list_agents_1 = get_hc_client_command()
        .args(["call", "--port", &admin_port_1.to_string(), "list-agents"])
        .output()?;

    assert!(
        list_agents_1.status.success(),
        "list-agents failed: {:?} stderr: {}",
        list_agents_1.status,
        String::from_utf8_lossy(&list_agents_1.stderr)
    );
    let agents_1: Vec<serde_json::Value> = serde_json::from_slice(&list_agents_1.stdout)?;
    assert!(!agents_1.is_empty(), "Conductor 1 should have agent info");

    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn list_dnas_with_origin() -> Result<()> {
    // Create a conductor with restricted allowed origins
    let mut config = SweetConductorConfig::standard();

    // Set allowed origins to only accept "test-origin"
    config.admin_interfaces = Some(vec![AdminInterfaceConfig {
        driver: InterfaceDriver::Websocket {
            port: 0,
            danger_bind_addr: None,
            allowed_origins: AllowedOrigins::Origins(
                vec!["test-origin".to_string()].into_iter().collect(),
            ),
        },
    }]);

    let mut conductor = SweetConductor::from_config(config).await;
    let (dna, _, _) = SweetDnaFile::unique_from_inline_zomes(simple_crud_zome()).await;
    let expected_hash = dna.dna_hash().to_string();

    conductor.setup_app("app", &[dna]).await?;

    let admin_port = conductor
        .get_arbitrary_admin_websocket_port()
        .expect("admin port");

    // Test that the call fails without an origin
    let output_no_origin = get_hc_client_command()
        .args(["call", "--port", &admin_port.to_string(), "list-dnas"])
        .output()?;

    assert!(
        !output_no_origin.status.success(),
        "Expected call to fail without origin, but it succeeded. stderr: {}",
        String::from_utf8_lossy(&output_no_origin.stderr)
    );

    // Test that the call fails with wrong origin
    let output_wrong_origin = get_hc_client_command()
        .args([
            "call",
            "--port",
            &admin_port.to_string(),
            "--origin",
            "wrong-origin",
            "list-dnas",
        ])
        .output()?;

    assert!(
        !output_wrong_origin.status.success(),
        "Expected call to fail with wrong origin, but it succeeded. stderr: {}",
        String::from_utf8_lossy(&output_wrong_origin.stderr)
    );

    // Test that the call succeeds with correct origin
    let output_correct_origin = get_hc_client_command()
        .args([
            "call",
            "--port",
            &admin_port.to_string(),
            "--origin",
            "test-origin",
            "list-dnas",
        ])
        .output()?;

    assert!(
        output_correct_origin.status.success(),
        "Expected call to succeed with correct origin, but it failed. exit: {:?}, stderr: {}",
        output_correct_origin.status,
        String::from_utf8_lossy(&output_correct_origin.stderr)
    );

    let hashes: Vec<String> = serde_json::from_slice(&output_correct_origin.stdout)?;
    assert_eq!(hashes, vec![expected_hash]);

    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn peer_meta_info() -> Result<()> {
    ensure_fixture_packaged().await?;

    let conductor = SweetConductor::from_standard_config().await;

    // Install the fixture app (which contains multiple DNAs)
    let happ_path = fixture_path(["my-app", "my-fixture-app.happ"])?;
    let admin_port = conductor
        .get_arbitrary_admin_websocket_port()
        .expect("admin port");

    let install_output = get_hc_client_command()
        .args([
            "call",
            "--port",
            &admin_port.to_string(),
            "install-app",
            "--app-id",
            "peer-test-app",
            happ_path.to_str().unwrap(),
        ])
        .output()?;

    assert!(
        install_output.status.success(),
        "install-app failed: {:?} stderr: {}",
        install_output.status,
        String::from_utf8_lossy(&install_output.stderr)
    );

    // Get the list of DNAs to verify against peer-meta-info output
    let list_dnas_output = get_hc_client_command()
        .args(["call", "--port", &admin_port.to_string(), "list-dnas"])
        .output()?;

    assert!(list_dnas_output.status.success());
    let mut dna_hashes: Vec<String> = serde_json::from_slice(&list_dnas_output.stdout)?;
    dna_hashes.sort();

    // Test getting peer meta info for all DNAs
    let peer_meta_all = get_hc_client_command()
        .args([
            "call",
            "--port",
            &admin_port.to_string(),
            "peer-meta-info",
            "--url",
            "wss://test-url:443",
        ])
        .output()?;

    assert!(
        peer_meta_all.status.success(),
        "peer-meta-info (all DNAs) failed: {:?} stderr: {}",
        peer_meta_all.status,
        String::from_utf8_lossy(&peer_meta_all.stderr)
    );

    // Verify output structure - should be a map of DNA hash -> empty map (no peers yet)
    let peer_info_all: BTreeMap<String, BTreeMap<String, serde_json::Value>> =
        serde_json::from_slice(&peer_meta_all.stdout)?;

    // The fixture app has 2 unique DNAs
    assert_eq!(
        peer_info_all.len(),
        2,
        "Expected 2 DNAs in peer info output, got: {:?}",
        peer_info_all.keys()
    );

    // Verify that all DNA hashes from list-dnas appear in peer-meta-info
    for dna_hash in &dna_hashes {
        assert!(
            peer_info_all.contains_key(dna_hash),
            "DNA hash {dna_hash} not found in peer-meta-info output",
        );
    }

    // Test getting peer meta info for a specific DNA
    let first_dna = &dna_hashes[0];
    let peer_meta_single = get_hc_client_command()
        .args([
            "call",
            "--port",
            &admin_port.to_string(),
            "peer-meta-info",
            "--url",
            "wss://test-url:443",
            "--dna",
            first_dna,
        ])
        .output()?;

    assert!(
        peer_meta_single.status.success(),
        "peer-meta-info (single DNA) failed: {:?} stderr: {}",
        peer_meta_single.status,
        String::from_utf8_lossy(&peer_meta_single.stderr)
    );

    // Verify output structure - should contain only the requested DNA
    let peer_info_single: BTreeMap<String, BTreeMap<String, serde_json::Value>> =
        serde_json::from_slice(&peer_meta_single.stdout)?;

    assert_eq!(
        peer_info_single.len(),
        1,
        "Expected 1 DNA in peer info output for specific query"
    );
    assert!(
        peer_info_single.contains_key(first_dna),
        "Requested DNA hash {first_dna} not found in output",
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn zome_call_auth() -> Result<()> {
    ensure_fixture_packaged().await?;

    let conductor = SweetConductor::from_standard_config().await;

    // Install the fixture app
    let happ_path = fixture_path(["my-app", "my-fixture-app.happ"])?;
    let admin_port = conductor
        .get_arbitrary_admin_websocket_port()
        .expect("admin port");

    let install_output = get_hc_client_command()
        .args([
            "call",
            "--port",
            &admin_port.to_string(),
            "install-app",
            "--app-id",
            "auth-test-app",
            happ_path.to_str().unwrap(),
        ])
        .output()?;

    assert!(
        install_output.status.success(),
        "install-app failed: {:?} stderr: {}",
        install_output.status,
        String::from_utf8_lossy(&install_output.stderr)
    );

    // Create a temp directory for the auth file
    let temp_dir = tempfile::TempDir::new()?;
    let auth_file = temp_dir.path().join(".hc_auth");

    // Generate signing credentials using zome-call-auth with piped passphrase
    let mut auth_cmd = TokioCommand::new(get_target("hc-client"))
        .args([
            "zome-call-auth",
            "--port",
            &admin_port.to_string(),
            "--piped",
            "auth-test-app",
        ])
        .current_dir(temp_dir.path())
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()?;

    // Write the passphrase to stdin
    let mut stdin = auth_cmd.stdin.take().expect("Failed to get stdin");
    stdin.write_all(b"test-passphrase\n").await?;
    drop(stdin);

    let output = auth_cmd.wait_with_output().await?;

    assert!(
        output.status.success(),
        "zome-call-auth failed: {:?}\nstdout: {}\nstderr: {}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    // Verify the auth file was created
    assert!(
        auth_file.exists(),
        "Auth file should have been created at {auth_file:?}",
    );

    // Verify the auth file contains valid data (should be non-empty)
    let auth_data = std::fs::read(&auth_file)?;
    assert!(
        !auth_data.is_empty(),
        "Auth file should contain credential data"
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn zome_call() -> Result<()> {
    ensure_fixture_packaged().await?;

    let conductor = SweetConductor::from_standard_config().await;

    // Install the fixture app
    let happ_path = fixture_path(["my-app", "my-fixture-app.happ"])?;
    let admin_port = conductor
        .get_arbitrary_admin_websocket_port()
        .expect("admin port");

    let install_output = get_hc_client_command()
        .args([
            "call",
            "--port",
            &admin_port.to_string(),
            "install-app",
            "--app-id",
            "zome-call-test-app",
            happ_path.to_str().unwrap(),
        ])
        .output()?;

    assert!(
        install_output.status.success(),
        "install-app failed: {:?} stderr: {}",
        install_output.status,
        String::from_utf8_lossy(&install_output.stderr)
    );

    // Get the DNA hash from the installed app
    let list_dnas_output = get_hc_client_command()
        .args(["call", "--port", &admin_port.to_string(), "list-dnas"])
        .output()?;

    assert!(list_dnas_output.status.success());
    let dna_hashes: Vec<String> = serde_json::from_slice(&list_dnas_output.stdout)?;
    let dna_hash = dna_hashes
        .first()
        .expect("Should have at least one DNA hash");

    // Create a temp directory for the auth file
    let temp_dir = tempfile::TempDir::new()?;

    // Generate signing credentials using zome-call-auth
    let mut auth_cmd = TokioCommand::new(get_target("hc-client"))
        .args([
            "zome-call-auth",
            "--port",
            &admin_port.to_string(),
            "--piped",
            "zome-call-test-app",
        ])
        .current_dir(temp_dir.path())
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()?;

    let mut stdin = auth_cmd.stdin.take().expect("Failed to get stdin");
    stdin.write_all(b"test-passphrase\n").await?;
    drop(stdin);

    let auth_output = auth_cmd.wait_with_output().await?;
    assert!(
        auth_output.status.success(),
        "zome-call-auth failed: {:?}\nstderr: {}",
        auth_output.status,
        String::from_utf8_lossy(&auth_output.stderr)
    );

    // Now make a zome call using the credentials
    let mut zome_call_cmd = TokioCommand::new(get_target("hc-client"))
        .args([
            "zome-call",
            "--port",
            &admin_port.to_string(),
            "--piped",
            "zome-call-test-app",
            dna_hash,
            "zome1",
            "foo",
            "null",
        ])
        .current_dir(temp_dir.path())
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()?;

    let mut stdin = zome_call_cmd.stdin.take().expect("Failed to get stdin");
    stdin.write_all(b"test-passphrase\n").await?;
    drop(stdin);

    let zome_call_output = zome_call_cmd.wait_with_output().await?;

    assert!(
        zome_call_output.status.success(),
        "zome-call failed: {:?}\nstdout: {}\nstderr: {}",
        zome_call_output.status,
        String::from_utf8_lossy(&zome_call_output.stdout),
        String::from_utf8_lossy(&zome_call_output.stderr)
    );

    // Verify the zome call returned the expected value
    let output_str = String::from_utf8_lossy(&zome_call_output.stdout);
    let trimmed = output_str.trim();

    // The foo function returns "foo" as a string, so we expect JSON-encoded "foo"
    assert_eq!(
        trimmed, "\"foo\"",
        "Expected zome call to return \"foo\", got: {trimmed}",
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn zome_call_returns_hash() -> Result<()> {
    ensure_fixture_packaged().await?;

    let conductor = SweetConductor::from_standard_config().await;

    // Install the fixture app
    let happ_path = fixture_path(["my-app", "my-fixture-app.happ"])?;
    let admin_port = conductor
        .get_arbitrary_admin_websocket_port()
        .expect("admin port");

    let install_output = get_hc_client_command()
        .args([
            "call",
            "--port",
            &admin_port.to_string(),
            "install-app",
            "--app-id",
            "hash-test-app",
            happ_path.to_str().unwrap(),
        ])
        .output()?;

    assert!(
        install_output.status.success(),
        "install-app failed: {:?} stderr: {}",
        install_output.status,
        String::from_utf8_lossy(&install_output.stderr)
    );

    // Get the DNA hash from the installed app
    let list_dnas_output = get_hc_client_command()
        .args(["call", "--port", &admin_port.to_string(), "list-dnas"])
        .output()?;

    assert!(list_dnas_output.status.success());
    let dna_hashes: Vec<String> = serde_json::from_slice(&list_dnas_output.stdout)?;
    let dna_hash_str = dna_hashes
        .first()
        .expect("Should have at least one DNA hash");

    // Create a temp directory for the auth file
    let temp_dir = tempfile::TempDir::new()?;

    // Generate signing credentials using zome-call-auth
    let mut auth_cmd = TokioCommand::new(get_target("hc-client"))
        .args([
            "zome-call-auth",
            "--port",
            &admin_port.to_string(),
            "--piped",
            "hash-test-app",
        ])
        .current_dir(temp_dir.path())
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()?;

    let mut stdin = auth_cmd.stdin.take().expect("Failed to get stdin");
    stdin.write_all(b"test-passphrase\n").await?;
    drop(stdin);

    let auth_output = auth_cmd.wait_with_output().await?;
    assert!(
        auth_output.status.success(),
        "zome-call-auth failed: {:?}\nstderr: {}",
        auth_output.status,
        String::from_utf8_lossy(&auth_output.stderr)
    );

    // Call the get_dna_hash function that returns a DNA hash
    let mut zome_call_cmd = TokioCommand::new(get_target("hc-client"))
        .args([
            "zome-call",
            "--port",
            &admin_port.to_string(),
            "--piped",
            "hash-test-app",
            dna_hash_str,
            "zome1",
            "get_dna_hash",
            "null",
        ])
        .current_dir(temp_dir.path())
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()?;

    let mut stdin = zome_call_cmd.stdin.take().expect("Failed to get stdin");
    stdin.write_all(b"test-passphrase\n").await?;
    drop(stdin);

    let zome_call_output = zome_call_cmd.wait_with_output().await?;

    assert!(
        zome_call_output.status.success(),
        "zome-call failed: {:?}\nstdout: {}\nstderr: {}",
        zome_call_output.status,
        String::from_utf8_lossy(&zome_call_output.stdout),
        String::from_utf8_lossy(&zome_call_output.stderr)
    );

    // Parse the output - the get_dna_hash function returns a DnaHash which is serialized as bytes
    let output_str = String::from_utf8_lossy(&zome_call_output.stdout);

    // The output should be a JSON array of bytes representing the hash
    // Extract the byte array from the output like [1,2,3,...]
    let bytes_str = output_str
        .trim()
        .strip_prefix('[')
        .and_then(|s| s.strip_suffix(']'))
        .expect("Output should be a JSON array");

    let bytes: Vec<u8> = bytes_str
        .split(',')
        .map(|s| s.trim().parse::<u8>())
        .collect::<std::result::Result<Vec<u8>, _>>()?;

    // Parse the returned hash
    let returned_hash = DnaHash::from_raw_39(bytes);

    // Parse the expected hash from the string
    let expected_hash: DnaHashB64 = dna_hash_str.parse()?;
    let expected_hash: DnaHash = expected_hash.into();

    assert_eq!(
        returned_hash, expected_hash,
        "Returned DNA hash should match the expected hash"
    );

    Ok(())
}
