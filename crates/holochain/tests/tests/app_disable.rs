use holo_hash::{ActionHash, AgentPubKeyB64, DnaHashB64};
use holochain::{
    conductor::state::{AppInterfaceId, ConductorState},
    prelude::DisabledAppReason,
    retry_until_timeout,
    sweettest::{SweetConductor, SweetDnaFile},
};
use holochain_wasm_test_utils::TestWasm;
use serde::{Deserialize, Serialize};

/// Test that space is removed and cleaned up on app disable
#[tokio::test(flavor = "multi_thread")]
async fn space_removed_on_disable() {
    holochain_trace::test_run();

    let (dna_file, _, _) = SweetDnaFile::unique_from_test_wasms(vec![TestWasm::Create]).await;
    let mut conductor = SweetConductor::from_standard_config().await;
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

    // Disable the app
    let app_id = "test_app".to_string();
    conductor
        .clone()
        .disable_app(app_id, DisabledAppReason::User)
        .await
        .unwrap();

    // After disabling, verify that operations on this space fail
    // This confirms cells were removed from conductor state
    let result = holochain_p2p
        .publish(
            dna_hash.clone(),
            ActionHash::from_raw_36(vec![0; 36]).into(),
            cell.agent_pubkey().clone(),
            vec![],
            None,
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
