use holo_hash::ActionHash;
use holochain::{
    retry_until_timeout,
    sweettest::{SweetConductor, SweetDnaFile},
};
use holochain_wasm_test_utils::TestWasm;

/// Test that space is removed and cleaned up on app uninstall
#[tokio::test(flavor = "multi_thread")]
async fn space_removed_on_uninstall() {
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

    // Uninstall the app - this should call remove_cells_by_id
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
            None,
        )
        .await;

    // Should error because space was removed when cells were cleaned up
    assert!(result.is_err());

    // Verify space was removed
    let spaces = holochain_p2p.test_kitsune().list_spaces();
    assert_eq!(spaces.len(), 0);
}
