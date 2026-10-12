use holo_hash::{ActionHash, AgentPubKeyB64, DnaHashB64};
use holochain::{
    retry_until_timeout,
    sweettest::{SweetConductor, SweetDnaFile},
};
use holochain_conductor_api::state::{AppInterfaceId, ConductorState};
use holochain_wasm_test_utils::TestWasm;
use serde::{Deserialize, Serialize};

/// Test that space is removed and cleaned up on app uninstall
#[tokio::test(flavor = "multi_thread")]
async fn space_removed_on_uninstall() {
    holochain_trace::test_run();

    let (dna_file, _, _) = SweetDnaFile::unique_from_test_wasms(vec![TestWasm::Create]).await;
    let mut conductor = SweetConductor::standard().await;
    let app = conductor.setup_app("test_app", [&dna_file]).await.unwrap();
    let cells = app.into_cells();
    let cell = &cells[0];
    let dna_hash = cell.cell_id().dna_hash().clone();
    let holochain_p2p = conductor.holochain_p2p().clone();

    // Wait for space to be created
    retry_until_timeout!({
        let spaces = holochain_p2p.test_kitsune().list_spaces();
        if spaces.len() == 1 {
            break;
        }
    });

    // Verify space exists
    let spaces = holochain_p2p.test_kitsune().list_spaces();
    assert_eq!(spaces.len(), 1);

    // Uninstall the app
    let app_id = "test_app".to_string();
    conductor
        .clone()
        .uninstall_app(&app_id, false)
        .await
        .unwrap();

    // After uninstalling, verify that operations on this space fail
    // This confirms cells were removed from conductor state
    let result = holochain_p2p
        .publish(
            dna_hash.clone(),
            ActionHash::from_raw_36(vec![0; 36]).into(),
            cell.agent_pubkey().clone(),
            vec![],
            None,
        )
        .await;

    // Should error because space was removed when cells were cleaned up
    assert!(result.is_err());

    // Verify space was removed
    let spaces = holochain_p2p.test_kitsune().list_spaces();
    assert_eq!(spaces.len(), 0);

    // Verify conductor has no running cells
    #[derive(Serialize, Deserialize, Debug)]
    pub struct ConductorSerialized {
        running_cells: Vec<(DnaHashB64, AgentPubKeyB64)>,
        shutting_down: bool,
        admin_websocket_ports: Vec<u16>,
        app_interfaces: Vec<AppInterfaceId>,
    }
    #[derive(Serialize, Deserialize, Debug)]
    struct ConductorDump {
        conductor: ConductorSerialized,
        state: ConductorState,
    }
    let conductor_state: ConductorDump =
        serde_json::from_str(conductor.dump_conductor_state().await.unwrap().as_str()).unwrap();

    assert_eq!(conductor_state.conductor.running_cells.len(), 0);
}

/// Names of files in `dir` whose name contains `needle`, sorted.
fn files_containing(dir: &std::path::Path, needle: &str) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(dir)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .filter(|name| name.contains(needle))
        .collect();
    names.sort();
    names
}

/// Uninstalling the only app using a DNA removes its database files and permits reinstall.
#[tokio::test(flavor = "multi_thread")]
async fn database_files_removed_on_uninstall_and_reinstall_succeeds() {
    holochain_trace::test_run();

    let (dna_file, _, _) = SweetDnaFile::unique_from_test_wasms(vec![TestWasm::Create]).await;
    let dna_hash = dna_file.dna_hash().clone();
    let dna = dna_hash.to_string();
    let mut conductor = SweetConductor::standard().await;
    let app_id = "test_app".to_string();
    let app = conductor.setup_app(&app_id, [&dna_file]).await.unwrap();
    let agent = app.agent().clone();
    let db_dir = conductor.db_path().join("databases");

    let dht_file = format!("dht-{dna}.db");
    let peer_meta_file = format!("p2p-peer-meta-{dna}.db");
    let files = files_containing(&db_dir, &dna);
    assert!(files.contains(&dht_file));
    assert!(files.contains(&peer_meta_file));
    assert!(conductor
        .list_dna_hashes()
        .await
        .unwrap()
        .contains(&dna_hash));

    conductor
        .clone()
        .uninstall_app(&app_id, false)
        .await
        .unwrap();

    assert_eq!(
        files_containing(&db_dir, &dna),
        Vec::<String>::new(),
        "every per-DNA database file, including -wal and -shm, must be removed"
    );
    assert!(!conductor
        .list_dna_hashes()
        .await
        .unwrap()
        .contains(&dna_hash));

    // Reinstall the same DNA for the same agent: it must register the DNA
    // again and the cell must be callable.
    let app = conductor
        .setup_app_for_agent(&app_id, agent, [&dna_file])
        .await
        .unwrap();
    let zome = app.cells()[0].zome(TestWasm::Create.coordinator_zome_name());
    let _: ActionHash = conductor.call(&zome, "create_entry", ()).await;

    assert!(conductor
        .list_dna_hashes()
        .await
        .unwrap()
        .contains(&dna_hash));
    let files = files_containing(&db_dir, &dna);
    assert!(files.contains(&dht_file));
    assert!(files.contains(&peer_meta_file));
}
