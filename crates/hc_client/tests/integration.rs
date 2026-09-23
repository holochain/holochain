use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command as StdCommand;
use std::time::Duration;

use anyhow::{ensure, Result};
use holo_hash::{ActionHash, AgentPubKey, AgentPubKeyB64, DhtOpHash, DnaHash, DnaHashB64, HasHash};
use holochain::{sweettest::*, test_utils::inline_zomes::simple_crud_zome};
use holochain_client::AdminWebsocket;
use holochain_conductor_api::{
    AdminInterfaceConfig, AppInfo, CellInfo, DhtOpsCursor, FullStateDump, InterfaceDriver,
};
use holochain_types::op::{DhtOp, DhtOpHashed};
use holochain_types::prelude::{ActionData, AppStatus, CellId, DisabledAppReason};
use holochain_types::websocket::AllowedOrigins;
use std::collections::{BTreeMap, HashSet};
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
    let mut conductor = SweetConductor::standard().await;
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
async fn dump_commands_return_single_cursor_page() -> Result<()> {
    let mut conductor = SweetConductor::standard().await;
    let (dna, _, _) = SweetDnaFile::unique_from_inline_zomes(simple_crud_zome()).await;
    let app = conductor.setup_app("app", &[dna]).await?;
    let cell_id = app.cells()[0].cell_id().clone();
    let dna = cell_id.dna_hash().to_string();
    let agent = cell_id.agent_pubkey().to_string();
    let admin_port = conductor
        .get_arbitrary_admin_websocket_port()
        .expect("admin port");
    let admin_port_arg = admin_port.to_string();
    let admin_ws = AdminWebsocket::connect(format!("127.0.0.1:{admin_port}"), None).await?;

    let unbounded_source_json: serde_json::Value =
        serde_json::from_str(&admin_ws.dump_state(cell_id.clone(), None, None).await?)?;
    let unbounded_records = unbounded_source_json[0]["source_chain_dump"]["records"]
        .as_array()
        .expect("unbounded source-chain records")
        .clone();
    assert!(unbounded_source_json[1]
        .as_str()
        .expect("source-chain summary")
        .contains("Records returned:"));
    let unbounded_full = admin_ws
        .dump_full_state(cell_id.clone(), None, None)
        .await?;

    let first_source_page = get_hc_client_command()
        .args([
            "call",
            "--port",
            admin_port_arg.as_str(),
            "dump-state",
            dna.as_str(),
            agent.as_str(),
            "--limit",
            "2",
        ])
        .output()?;
    assert!(first_source_page.status.success());
    assert_eq!(
        String::from_utf8_lossy(&first_source_page.stdout)
            .lines()
            .count(),
        1
    );
    let first_source_json: serde_json::Value = serde_json::from_slice(&first_source_page.stdout)?;
    let first_records = first_source_json[0]["source_chain_dump"]["records"]
        .as_array()
        .expect("source-chain records")
        .clone();
    assert_eq!(first_records.len(), 2);
    assert!(first_source_json[1]
        .as_str()
        .expect("paginated source-chain summary")
        .contains("Records returned: 2"));

    let second_source_page = get_hc_client_command()
        .args([
            "call",
            "--port",
            admin_port_arg.as_str(),
            "dump-state",
            dna.as_str(),
            agent.as_str(),
            "--limit",
            "2",
            "--cursor",
            "1",
        ])
        .output()?;
    assert!(second_source_page.status.success());
    let second_source_json: serde_json::Value = serde_json::from_slice(&second_source_page.stdout)?;
    assert_eq!(
        String::from_utf8_lossy(&second_source_page.stdout)
            .lines()
            .count(),
        1
    );
    let second_records = second_source_json[0]["source_chain_dump"]["records"]
        .as_array()
        .expect("source-chain records")
        .clone();
    assert!(!second_records.is_empty());
    assert_ne!(
        first_records.last().unwrap()["action_address"],
        second_records.first().unwrap()["action_address"]
    );

    let combined_records: Vec<_> = first_records
        .iter()
        .chain(&second_records)
        .cloned()
        .collect();
    assert_eq!(combined_records, unbounded_records);

    let source_hash_cursor: ActionHash =
        serde_json::from_value(first_records.last().unwrap()["action_address"].clone())?;
    let hash_source_page = get_hc_client_command()
        .args([
            "call",
            "--port",
            admin_port_arg.as_str(),
            "dump-state",
            dna.as_str(),
            agent.as_str(),
            "--limit",
            "2",
            "--cursor",
            source_hash_cursor.to_string().as_str(),
        ])
        .output()?;
    assert!(hash_source_page.status.success());
    assert_eq!(
        String::from_utf8_lossy(&hash_source_page.stdout)
            .lines()
            .count(),
        1
    );
    let hash_source_json: serde_json::Value = serde_json::from_slice(&hash_source_page.stdout)?;
    assert_eq!(
        hash_source_json[0]["source_chain_dump"]["records"],
        second_source_json[0]["source_chain_dump"]["records"]
    );

    let expected_hashes: HashSet<_> = unbounded_full
        .integration_dump
        .validation_limbo
        .iter()
        .chain(&unbounded_full.integration_dump.integration_limbo)
        .chain(&unbounded_full.integration_dump.integrated)
        .cloned()
        .map(DhtOpHashed::from_content_sync)
        .map(|op| op.as_hash().clone())
        .collect();
    let mut actual_hashes = Vec::new();
    let mut cursor: Option<DhtOpsCursor> = None;
    let mut page_index = 0;
    loop {
        let mut command = get_hc_client_command();
        command.args([
            "call",
            "--port",
            admin_port_arg.as_str(),
            "dump-full-state",
            dna.as_str(),
            agent.as_str(),
            "--limit",
            "2",
        ]);
        if let Some(cursor) = &cursor {
            command.args([
                "--cursor",
                cursor.when_received.to_string().as_str(),
                cursor.hash.to_string().as_str(),
            ]);
        }
        let page_output = command.output()?;
        assert!(page_output.status.success());
        assert_eq!(
            String::from_utf8_lossy(&page_output.stdout).lines().count(),
            1
        );
        let page: FullStateDump = serde_json::from_slice(&page_output.stdout)?;
        if page_index == 0 {
            assert_eq!(page.peer_dump, unbounded_full.peer_dump);
            assert_eq!(page.source_chain_dump, unbounded_full.source_chain_dump);
        }
        let page_ops = page
            .integration_dump
            .validation_limbo
            .into_iter()
            .chain(page.integration_dump.integration_limbo)
            .chain(page.integration_dump.integrated)
            .collect::<Vec<DhtOp>>();
        assert!(page_ops.len() <= 2);
        actual_hashes.extend(
            page_ops
                .into_iter()
                .map(DhtOpHashed::from_content_sync)
                .map(|op| op.as_hash().clone()),
        );
        cursor = page.integration_dump.dht_ops_cursor;
        page_index += 1;
        if cursor.is_none() {
            break;
        }
    }
    assert!(page_index > 1);
    assert_eq!(actual_hashes.len(), expected_hashes.len());
    assert_eq!(
        actual_hashes.into_iter().collect::<HashSet<_>>(),
        expected_hashes
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn dump_op_timings_command_pages() -> Result<()> {
    let mut conductor = SweetConductor::standard().await;
    let (dna, _, _) = SweetDnaFile::unique_from_inline_zomes(simple_crud_zome()).await;
    let app = conductor.setup_app("app", &[dna]).await?;
    let cell_id = app.cells()[0].cell_id().clone();
    let dna_hash = cell_id.dna_hash().clone();
    let dna = dna_hash.to_string();
    let admin_port = conductor
        .get_arbitrary_admin_websocket_port()
        .expect("admin port");
    let admin_port_arg = admin_port.to_string();
    let admin_ws = AdminWebsocket::connect(format!("127.0.0.1:{admin_port}"), None).await?;

    let unbounded = admin_ws.dump_op_timings(dna_hash, None, None).await?;
    ensure!(
        unbounded.timings.len() >= 2,
        "expected the genesis ops to be dumped"
    );

    let first_page = get_hc_client_command()
        .args([
            "call",
            "--port",
            admin_port_arg.as_str(),
            "dump-op-timings",
            dna.as_str(),
            "--limit",
            "1",
        ])
        .output()?;
    ensure!(first_page.status.success(), "dump-op-timings failed");

    let first_json: serde_json::Value = serde_json::from_slice(&first_page.stdout)?;
    let timings = first_json["timings"]
        .as_array()
        .expect("timings array")
        .clone();
    ensure!(timings.len() == 1, "limit was not applied");
    ensure!(
        timings[0]["when_received"].is_i64(),
        "received time is reported"
    );

    let when_received = first_json["cursor"]["when_received"]
        .as_i64()
        .expect("cursor received time")
        .to_string();
    // `DhtOpHash` serializes to JSON as a byte array (see `holo_hash::ser`), not a
    // base64 string, so it must be decoded before it can be passed back to the CLI,
    // which expects the base64 string form accepted by `DhtOpHash::try_from(&str)`.
    let cursor_hash: DhtOpHash = serde_json::from_value(first_json["cursor"]["hash"].clone())?;
    let cursor_hash = cursor_hash.to_string();

    let second_page = get_hc_client_command()
        .args([
            "call",
            "--port",
            admin_port_arg.as_str(),
            "dump-op-timings",
            dna.as_str(),
            "--limit",
            "1",
            "--cursor",
            when_received.as_str(),
            cursor_hash.as_str(),
        ])
        .output()?;
    ensure!(second_page.status.success(), "paged dump-op-timings failed");

    let second_json: serde_json::Value = serde_json::from_slice(&second_page.stdout)?;
    let second_timings = second_json["timings"].as_array().expect("timings array");
    ensure!(
        second_timings.len() == 1,
        "second page limit was not applied"
    );
    ensure!(
        second_timings[0]["op_hash"] != timings[0]["op_hash"],
        "cursor is exclusive"
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn list_apps() -> Result<()> {
    let mut conductor = SweetConductor::standard().await;
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
    let conductor = SweetConductor::standard().await;

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
    let conductor = SweetConductor::standard().await;

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
    let conductor = SweetConductor::standard().await;

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
async fn install_app_membrane_proof_flags_reach_genesis() -> Result<()> {
    let conductor = SweetConductor::standard().await;
    let admin_port = conductor
        .get_arbitrary_admin_websocket_port()
        .expect("admin port");
    let temp_dir = tempfile::TempDir::new()?;
    let fixture = package_membrane_proof_fixture(temp_dir.path(), false).await?;

    let new_agent = TokioCommand::new(get_target("hc-client"))
        .args(["call", "--port", &admin_port.to_string(), "new-agent"])
        .output()
        .await?;
    ensure!(
        new_agent.status.success(),
        "new-agent failed: {}",
        String::from_utf8_lossy(&new_agent.stderr)
    );
    let agent_key_text: String = serde_json::from_slice(&new_agent.stdout)?;
    let agent_key: AgentPubKey = agent_key_text.parse::<AgentPubKeyB64>()?.into();

    let role_1_proof = vec![0x00, 0xff, 0x80, 0x0a];
    let role_2_proof = vec![0x7f, 0x00, 0xfe, 0x0d];
    fs::write(&fixture.flag_role_1_proof, &role_1_proof)?;
    fs::write(&fixture.flag_role_2_proof, &role_2_proof)?;

    let install = TokioCommand::new(get_target("hc-client"))
        .args([
            "call",
            "--port",
            &admin_port.to_string(),
            "install-app",
            "--app-id",
            "proof-flags",
            "--agent-key",
            &agent_key.to_string(),
            "--membrane-proof",
            "role-1=flag-role-1.bin",
            "--membrane-proof",
            "role-2=flag-role-2.bin",
            fixture.app_bundle.to_str().unwrap(),
        ])
        .current_dir(temp_dir.path())
        .output()
        .await?;
    ensure!(
        install.status.success(),
        "install-app with membrane proofs failed: {}",
        String::from_utf8_lossy(&install.stderr)
    );
    let app_json: serde_json::Value = serde_json::from_slice(&install.stdout)?;
    assert_eq!(app_json["installed_app_id"], "proof-flags");
    let admin_ws = AdminWebsocket::connect(format!("127.0.0.1:{admin_port}"), None).await?;
    let app_info = admin_ws
        .list_apps(None)
        .await?
        .into_iter()
        .find(|app| app.installed_app_id == "proof-flags")
        .ok_or_else(|| anyhow::anyhow!("installed app missing from admin list"))?;
    assert_eq!(app_info.agent_pub_key, agent_key);

    assert_role_proof(&admin_ws, &app_info, "role-1", &role_1_proof, true).await?;
    assert_role_proof(&admin_ws, &app_info, "role-2", &role_2_proof, true).await?;
    assert_role_proof(&admin_ws, &app_info, "role-3", &[], false).await?;

    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn install_app_membrane_proof_yaml_sources_and_errors() -> Result<()> {
    let conductor = SweetConductor::standard().await;
    let admin_port = conductor
        .get_arbitrary_admin_websocket_port()
        .expect("admin port");
    let temp_dir = tempfile::TempDir::new()?;
    let fixture = package_membrane_proof_fixture(temp_dir.path(), false).await?;
    let role_1_proof = vec![0x01, 0xff, 0x80, 0x0b];
    let role_2_proof = vec![0x02, 0xfe, 0x81, 0x0c];
    fs::write(&fixture.yaml_role_2_proof, &role_2_proof)?;
    let command_dir = temp_dir.path().join("different-working-directory");
    fs::create_dir_all(&command_dir)?;
    fs::write(
        &fixture.roles_settings,
        "role-1:\n  type: provisioned\n  membrane_proof:\n    base64: Af+ACw==\n  modifiers:\n    network_seed: yaml-role-1\nrole-2:\n  type: provisioned\n  membrane_proof:\n    path: yaml-role-2.bin\n  modifiers:\n    network_seed: yaml-role-2\nrole-3:\n  type: provisioned\n  modifiers:\n    network_seed: yaml-role-3\n",
    )?;

    let install = TokioCommand::new(get_target("hc-client"))
        .args([
            "call",
            "--port",
            &admin_port.to_string(),
            "install-app",
            "--app-id",
            "proof-yaml",
            fixture.app_bundle.to_str().unwrap(),
            "global-yaml-seed",
            fixture.roles_settings.to_str().unwrap(),
        ])
        .current_dir(&command_dir)
        .output()
        .await?;
    ensure!(
        install.status.success(),
        "install-app with YAML proofs failed: {}",
        String::from_utf8_lossy(&install.stderr)
    );
    let app_json: serde_json::Value = serde_json::from_slice(&install.stdout)?;
    assert_eq!(app_json["installed_app_id"], "proof-yaml");
    let admin_ws = AdminWebsocket::connect(format!("127.0.0.1:{admin_port}"), None).await?;
    let app_info = admin_ws
        .list_apps(None)
        .await?
        .into_iter()
        .find(|app| app.installed_app_id == "proof-yaml")
        .ok_or_else(|| anyhow::anyhow!("installed app missing from admin list"))?;
    assert_role_proof(&admin_ws, &app_info, "role-1", &role_1_proof, true).await?;
    assert_role_proof(&admin_ws, &app_info, "role-2", &role_2_proof, true).await?;
    assert_role_proof(&admin_ws, &app_info, "role-3", &[], false).await?;

    let invalid_settings = fixture.fixture_dir.join("invalid-roles.yaml");
    fs::write(
        &invalid_settings,
        "role-1:\n  type: provisioned\n  membrane_proof:\n    base64: not valid !!!\n",
    )?;
    let invalid = TokioCommand::new(get_target("hc-client"))
        .args([
            "call",
            "--port",
            &admin_port.to_string(),
            "install-app",
            "--app-id",
            "proof-invalid",
            fixture.app_bundle.to_str().unwrap(),
            "invalid-seed",
            invalid_settings.to_str().unwrap(),
        ])
        .current_dir(temp_dir.path())
        .output()
        .await?;
    assert!(!invalid.status.success());
    let invalid_stderr = String::from_utf8_lossy(&invalid.stderr);
    assert!(invalid_stderr.contains("membrane_proof"));
    assert!(invalid_stderr.contains("role-1"));

    let conflict_settings = fixture.fixture_dir.join("conflict-roles.yaml");
    fs::write(
        &conflict_settings,
        "role-1:\n  type: provisioned\n  membrane_proof:\n    base64: AQID\n",
    )?;
    fs::write(temp_dir.path().join("conflict-proof.bin"), [0x01, 0x02])?;
    let conflict = TokioCommand::new(get_target("hc-client"))
        .args([
            "call",
            "--port",
            &admin_port.to_string(),
            "install-app",
            "--app-id",
            "proof-conflict",
            "--membrane-proof",
            "role-1=conflict-proof.bin",
            fixture.app_bundle.to_str().unwrap(),
            "conflict-seed",
            conflict_settings.to_str().unwrap(),
        ])
        .current_dir(temp_dir.path())
        .output()
        .await?;
    assert!(!conflict.status.success());
    let conflict_stderr = String::from_utf8_lossy(&conflict.stderr);
    assert!(
        conflict_stderr.contains("membrane proof") || conflict_stderr.contains("proof"),
        "unexpected conflict error: {conflict_stderr}"
    );

    let apps = admin_ws.list_apps(None).await?;
    assert!(apps
        .iter()
        .all(|app| app.installed_app_id != "proof-invalid"));
    assert!(apps
        .iter()
        .all(|app| app.installed_app_id != "proof-conflict"));

    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn deferred_membrane_proofs_cli() -> Result<()> {
    let conductor = SweetConductor::standard().await;
    let admin_port = conductor
        .get_arbitrary_admin_websocket_port()
        .expect("admin port");
    let admin_ws = AdminWebsocket::connect(format!("127.0.0.1:{admin_port}"), None).await?;
    let temp_dir = tempfile::TempDir::new()?;
    let fixture = package_membrane_proof_fixture(temp_dir.path(), true).await?;
    let role_1_proof = vec![0x00, 0xff, 0x80, 0x0a];
    let role_2_proof = vec![0x7f, 0x00, 0xfe, 0x0d];
    fs::write(temp_dir.path().join("flag-role-1.bin"), &role_1_proof)?;
    fs::write(temp_dir.path().join("flag-role-2.bin"), &role_2_proof)?;

    let deferred_app_id = "deferred-flags";
    let install = run_client_command(
        vec![
            "call".into(),
            "--port".into(),
            admin_port.to_string(),
            "install-app".into(),
            "--app-id".into(),
            deferred_app_id.into(),
            fixture.app_bundle.to_str().unwrap().into(),
        ],
        temp_dir.path(),
    )
    .await?;
    ensure!(
        install.status.success(),
        "deferred install failed: {}",
        String::from_utf8_lossy(&install.stderr)
    );
    let original_info = app_from_admin(&admin_ws, deferred_app_id).await?;
    assert_eq!(original_info.status, AppStatus::AwaitingMemproofs);
    let original_agent = original_info.agent_pub_key.clone();

    let interfaces_before_errors = admin_ws.list_app_interfaces().await?;
    let missing = run_client_command(
        vec![
            "provide-memproofs".into(),
            "--port".into(),
            admin_port.to_string(),
            deferred_app_id.into(),
            "--membrane-proof".into(),
            "role-1=missing.bin".into(),
        ],
        temp_dir.path(),
    )
    .await?;
    assert!(!missing.status.success());
    assert!(String::from_utf8_lossy(&missing.stderr).contains("membrane_proof"));
    assert_eq!(
        admin_ws.list_app_interfaces().await?.len(),
        interfaces_before_errors.len()
    );
    assert_eq!(
        app_from_admin(&admin_ws, deferred_app_id).await?.status,
        AppStatus::AwaitingMemproofs
    );

    let invalid_yaml = temp_dir.path().join("invalid-proofs.yaml");
    fs::write(&invalid_yaml, "role-1:\n  base64: invalid base64 !!!\n")?;
    let invalid = run_client_command(
        vec![
            "provide-memproofs".into(),
            "--port".into(),
            admin_port.to_string(),
            deferred_app_id.into(),
            "--membrane-proofs".into(),
            invalid_yaml.to_str().unwrap().into(),
        ],
        temp_dir.path(),
    )
    .await?;
    assert!(!invalid.status.success());
    let invalid_stderr = String::from_utf8_lossy(&invalid.stderr);
    assert!(
        invalid_stderr.contains("base64") || invalid_stderr.contains("proof"),
        "unexpected invalid proof error: {invalid_stderr}"
    );
    assert_eq!(
        admin_ws.list_app_interfaces().await?.len(),
        interfaces_before_errors.len()
    );

    let unknown_role_yaml = temp_dir.path().join("unknown-role.yaml");
    fs::write(&unknown_role_yaml, "unknown-role:\n  base64: AQID\n")?;
    let unknown_role = run_client_command(
        vec![
            "provide-memproofs".into(),
            "--port".into(),
            admin_port.to_string(),
            deferred_app_id.into(),
            "--membrane-proofs".into(),
            unknown_role_yaml.to_str().unwrap().into(),
        ],
        temp_dir.path(),
    )
    .await?;
    assert!(!unknown_role.status.success());
    assert!(String::from_utf8_lossy(&unknown_role.stderr).contains("unknown membrane proof role"));
    assert_eq!(
        admin_ws.list_app_interfaces().await?.len(),
        interfaces_before_errors.len()
    );

    let wrong_app_interface = admin_ws
        .attach_app_interface(
            0,
            None,
            AllowedOrigins::Origins(vec!["sandbox".to_string()].into_iter().collect()),
            Some("another-app".to_string()),
        )
        .await?;
    let interfaces_before_flags = admin_ws.list_app_interfaces().await?;
    let flags = run_client_command(
        vec![
            "provide-memproofs".into(),
            "--port".into(),
            admin_port.to_string(),
            deferred_app_id.into(),
            "--membrane-proof".into(),
            "role-1=flag-role-1.bin".into(),
            "--membrane-proof".into(),
            "role-2=flag-role-2.bin".into(),
        ],
        temp_dir.path(),
    )
    .await?;
    ensure!(
        flags.status.success(),
        "provide-memproofs failed: {}",
        String::from_utf8_lossy(&flags.stderr)
    );
    let flags_output: serde_json::Value = serde_json::from_slice(&flags.stdout)?;
    assert_eq!(flags_output["installed_app_id"], deferred_app_id);
    assert_eq!(flags_output["status"]["type"], "disabled");
    assert_eq!(
        flags_output["status"]["value"]["type"],
        "not_started_after_providing_memproofs"
    );
    let after_flags = app_from_admin(&admin_ws, deferred_app_id).await?;
    assert_eq!(
        after_flags.status,
        AppStatus::Disabled(DisabledAppReason::NotStartedAfterProvidingMemproofs)
    );
    assert_eq!(after_flags.agent_pub_key, original_agent);
    let interfaces_after_flags = admin_ws.list_app_interfaces().await?;
    assert_eq!(
        interfaces_after_flags.len(),
        interfaces_before_flags.len() + 1
    );
    assert!(interfaces_after_flags
        .iter()
        .any(|interface| interface.port != wrong_app_interface));

    let second_submission = run_client_command(
        vec![
            "provide-memproofs".into(),
            "--port".into(),
            admin_port.to_string(),
            deferred_app_id.into(),
            "--membrane-proof".into(),
            "role-1=flag-role-1.bin".into(),
        ],
        temp_dir.path(),
    )
    .await?;
    assert!(!second_submission.status.success());
    assert!(String::from_utf8_lossy(&second_submission.stderr).contains("not awaiting"));
    assert_eq!(
        admin_ws.list_app_interfaces().await?.len(),
        interfaces_after_flags.len()
    );

    let enable = run_client_command(
        vec![
            "call".into(),
            "--port".into(),
            admin_port.to_string(),
            "enable-app".into(),
            deferred_app_id.into(),
        ],
        temp_dir.path(),
    )
    .await?;
    ensure!(
        enable.status.success(),
        "enable-app failed: {}",
        String::from_utf8_lossy(&enable.stderr)
    );
    assert_eq!(
        app_from_admin(&admin_ws, deferred_app_id).await?.status,
        AppStatus::Enabled
    );
    assert!(!temp_dir.path().join(".hc_auth").exists());
    let enabled_flags = app_from_admin(&admin_ws, deferred_app_id).await?;
    assert_role_proof(&admin_ws, &enabled_flags, "role-1", &role_1_proof, true).await?;
    assert_role_proof(&admin_ws, &enabled_flags, "role-2", &role_2_proof, true).await?;
    assert_role_proof(&admin_ws, &enabled_flags, "role-3", &[], false).await?;

    let yaml_app_id = "deferred-yaml";
    let yaml_proof_path = temp_dir.path().join("yaml-role-2.bin");
    fs::write(&yaml_proof_path, [0x11, 0xee, 0x82, 0x0e])?;
    let yaml_settings = temp_dir.path().join("deferred-proofs.yaml");
    fs::write(
        &yaml_settings,
        "role-1:\n  base64: Af+ACw==\nrole-2:\n  path: yaml-role-2.bin\n",
    )?;
    let yaml_install = run_client_command(
        vec![
            "call".into(),
            "--port".into(),
            admin_port.to_string(),
            "install-app".into(),
            "--app-id".into(),
            yaml_app_id.into(),
            fixture.app_bundle.to_str().unwrap().into(),
        ],
        temp_dir.path(),
    )
    .await?;
    ensure!(yaml_install.status.success());
    let yaml_before = app_from_admin(&admin_ws, yaml_app_id).await?;
    let yaml_interfaces_before = admin_ws.list_app_interfaces().await?;
    let target_interface = admin_ws
        .attach_app_interface(
            0,
            None,
            AllowedOrigins::Origins(vec!["sandbox".to_string()].into_iter().collect()),
            Some(yaml_app_id.to_string()),
        )
        .await?;
    let yaml = run_client_command(
        vec![
            "provide-memproofs".into(),
            "--port".into(),
            admin_port.to_string(),
            yaml_app_id.into(),
            "--membrane-proofs".into(),
            yaml_settings.to_str().unwrap().into(),
        ],
        temp_dir.path(),
    )
    .await?;
    ensure!(
        yaml.status.success(),
        "YAML provide-memproofs failed: {}",
        String::from_utf8_lossy(&yaml.stderr)
    );
    let yaml_after = app_from_admin(&admin_ws, yaml_app_id).await?;
    assert_eq!(
        yaml_after.status,
        AppStatus::Disabled(DisabledAppReason::NotStartedAfterProvidingMemproofs)
    );
    let yaml_enable = run_client_command(
        vec![
            "call".into(),
            "--port".into(),
            admin_port.to_string(),
            "enable-app".into(),
            yaml_app_id.into(),
        ],
        temp_dir.path(),
    )
    .await?;
    ensure!(yaml_enable.status.success());
    let yaml_after = app_from_admin(&admin_ws, yaml_app_id).await?;
    assert_eq!(yaml_after.status, AppStatus::Enabled);
    assert_ne!(yaml_after.cell_info, after_flags.cell_info);
    assert_eq!(yaml_after.agent_pub_key, yaml_before.agent_pub_key);
    assert_role_proof(
        &admin_ws,
        &yaml_after,
        "role-1",
        &[0x01, 0xff, 0x80, 0x0b],
        true,
    )
    .await?;
    assert_role_proof(
        &admin_ws,
        &yaml_after,
        "role-2",
        &[0x11, 0xee, 0x82, 0x0e],
        true,
    )
    .await?;
    assert_role_proof(&admin_ws, &yaml_after, "role-3", &[], false).await?;
    let yaml_interfaces_after = admin_ws.list_app_interfaces().await?;
    assert_eq!(
        yaml_interfaces_after.len(),
        yaml_interfaces_before.len() + 1
    );
    assert!(yaml_interfaces_after
        .iter()
        .any(|interface| interface.port == target_interface));

    let unknown_app = run_client_command(
        vec![
            "provide-memproofs".into(),
            "--port".into(),
            admin_port.to_string(),
            "missing-app".into(),
            "--membrane-proof".into(),
            "role-1=flag-role-1.bin".into(),
        ],
        temp_dir.path(),
    )
    .await?;
    assert!(!unknown_app.status.success());
    assert!(String::from_utf8_lossy(&unknown_app.stderr).contains("app not found"));

    let empty_app_id = "deferred-empty";
    let empty_install = run_client_command(
        vec![
            "call".into(),
            "--port".into(),
            admin_port.to_string(),
            "install-app".into(),
            "--app-id".into(),
            empty_app_id.into(),
            fixture.app_bundle.to_str().unwrap().into(),
        ],
        temp_dir.path(),
    )
    .await?;
    ensure!(empty_install.status.success());
    let empty_yaml = temp_dir.path().join("empty-proofs.yaml");
    fs::write(&empty_yaml, "{}\n")?;
    let empty = run_client_command(
        vec![
            "provide-memproofs".into(),
            "--port".into(),
            admin_port.to_string(),
            empty_app_id.into(),
            "--membrane-proofs".into(),
            empty_yaml.to_str().unwrap().into(),
        ],
        temp_dir.path(),
    )
    .await?;
    ensure!(empty.status.success());
    assert_eq!(
        app_from_admin(&admin_ws, empty_app_id).await?.status,
        AppStatus::Disabled(DisabledAppReason::NotStartedAfterProvidingMemproofs)
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn uninstall_app() -> Result<()> {
    let conductor = SweetConductor::standard().await;

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

struct MembraneProofFixture {
    fixture_dir: PathBuf,
    app_bundle: PathBuf,
    roles_settings: PathBuf,
    yaml_role_2_proof: PathBuf,
    flag_role_1_proof: PathBuf,
    flag_role_2_proof: PathBuf,
}

async fn package_membrane_proof_fixture(
    temp_dir: &Path,
    allow_deferred_memproofs: bool,
) -> Result<MembraneProofFixture> {
    let source = fixture_path(["my-app"])?;
    let fixture_dir = temp_dir.join("membrane-proof-fixture");
    let dna_dir = fixture_dir.join("dna");
    let zomes_dir = dna_dir.join("zomes");
    fs::create_dir_all(&zomes_dir)?;

    fs::copy(source.join("dna/dna.yaml"), dna_dir.join("dna.yaml"))?;
    fs::copy(
        source.join("dna/zomes/test_wasm_foo.wasm"),
        zomes_dir.join("test_wasm_foo.wasm"),
    )?;
    let mut manifest = fs::read_to_string(source.join("happ.yaml"))?
        .replacen("\"0123456\"", "\"proof-flag-role-1\"", 1)
        .replacen("\"0123456\"", "\"proof-flag-role-2\"", 1)
        .replace(
            "should remain untouched by roles settings test",
            "proof-flag-role-3",
        );
    if allow_deferred_memproofs {
        manifest = manifest.replace(
            "allow_deferred_memproofs: false",
            "allow_deferred_memproofs: true",
        );
    }
    fs::write(fixture_dir.join("happ.yaml"), manifest)?;

    let hc_bin = get_hc_command();
    let dna_status = TokioCommand::new(&hc_bin)
        .args(["dna", "pack"])
        .arg(&dna_dir)
        .status()
        .await?;
    ensure!(dna_status.success(), "failed to pack membrane proof DNA");
    let app_status = TokioCommand::new(&hc_bin)
        .args(["app", "pack"])
        .arg(&fixture_dir)
        .status()
        .await?;
    ensure!(app_status.success(), "failed to pack membrane proof hApp");

    Ok(MembraneProofFixture {
        app_bundle: fixture_dir.join("my-fixture-app.happ"),
        roles_settings: fixture_dir.join("roles.yaml"),
        yaml_role_2_proof: fixture_dir.join("yaml-role-2.bin"),
        flag_role_1_proof: temp_dir.join("flag-role-1.bin"),
        flag_role_2_proof: temp_dir.join("flag-role-2.bin"),
        fixture_dir,
    })
}

async fn assert_role_proof(
    admin_ws: &AdminWebsocket,
    app_info: &AppInfo,
    role: &str,
    expected: &[u8],
    should_have_proof: bool,
) -> Result<()> {
    let cells = app_info
        .cell_info
        .get(role)
        .ok_or_else(|| anyhow::anyhow!("role {role} missing from app info"))?;
    let CellInfo::Provisioned(cell) = cells
        .first()
        .ok_or_else(|| anyhow::anyhow!("role {role} has no provisioned cell"))?
    else {
        anyhow::bail!("role {role} does not have a provisioned cell");
    };
    let dump = admin_ws
        .dump_full_state(cell.cell_id.clone(), None, None)
        .await?;
    let proof =
        dump.source_chain_dump
            .records
            .iter()
            .find_map(|record| match &record.action.data {
                ActionData::AgentValidationPkg(pkg) => pkg
                    .membrane_proof
                    .as_ref()
                    .map(|proof| proof.bytes().to_vec()),
                _ => None,
            });
    if should_have_proof {
        assert_eq!(proof.as_deref(), Some(expected));
    } else {
        assert!(proof.is_none(), "role {role} unexpectedly has a proof");
    }
    Ok(())
}

async fn run_client_command(args: Vec<String>, current_dir: &Path) -> Result<std::process::Output> {
    Ok(TokioCommand::new(get_target("hc-client"))
        .args(args)
        .current_dir(current_dir)
        .stdin(std::process::Stdio::null())
        .output()
        .await?)
}

async fn app_from_admin(admin_ws: &AdminWebsocket, app_id: &str) -> Result<AppInfo> {
    admin_ws
        .list_apps(None)
        .await?
        .into_iter()
        .find(|app| app.installed_app_id == app_id)
        .ok_or_else(|| anyhow::anyhow!("installed app missing from admin list: {app_id}"))
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
    let conductor = SweetConductor::standard().await;

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
    let conductor = SweetConductor::standard().await;

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
    let mut conductors = SweetConductorBatch::standard(2).await;

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

    let mut conductor =
        SweetConductor::from_config_rendezvous(config, SweetLocalRendezvous::new().await).await;
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

    let conductor = SweetConductor::standard().await;

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

    let conductor = SweetConductor::standard().await;

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

    let conductor = SweetConductor::standard().await;

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

    let conductor = SweetConductor::standard().await;

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
