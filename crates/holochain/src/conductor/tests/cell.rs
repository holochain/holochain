use crate::prelude::InitCallbackResult;
use crate::sweettest::{SweetConductor, SweetDnaFile};
use crate::test_utils::retry_fn_until_timeout;
use holochain_wasm_test_utils::TestWasm;

#[tokio::test(flavor = "multi_thread")]
async fn check_or_run_zome_init_triggers_zome_initialization() {
    let zome = TestWasm::InitPass;
    let (dna, _, _) = SweetDnaFile::unique_from_test_wasms(vec![zome]).await;
    let mut conductor = SweetConductor::standard().await;
    let app = conductor.setup_app("app", [&dna]).await.unwrap();
    let cell_id = app.cells()[0].cell_id();

    // Wait for integration workflow to complete
    retry_fn_until_timeout(
        || async { conductor.all_ops_integrated(dna.dna_hash()).unwrap() },
        None,
        None,
    )
    .await
    .unwrap();

    // Take state dump before calling check_or_run_zome_init
    let before_state_dump = conductor
        .raw_handle()
        .dump_full_cell_state(cell_id, None)
        .await
        .expect("Failed to get state dump");

    // Init zomes, so that all genesis records are committed
    let cell = conductor
        .raw_handle()
        .cell_by_id(cell_id)
        .await
        .expect("failed to get cell");
    cell.check_or_run_zome_init()
        .await
        .expect("failed to init cell");

    // Wait for integration workflow to complete
    retry_fn_until_timeout(
        || async { conductor.all_ops_integrated(dna.dna_hash()).unwrap() },
        None,
        None,
    )
    .await
    .unwrap();

    // Take state dump after calling check_or_run_zome_init
    let after_state_dump = conductor
        .raw_handle()
        .dump_full_cell_state(cell_id, None)
        .await
        .expect("Failed to get state dump");

    // 2 new DhtOps for InitZomesComplete are integrated into the source chain
    assert_eq!(
        after_state_dump.integration_dump.integrated.len()
            - before_state_dump.integration_dump.integrated.len(),
        2
    );
    assert_eq!(after_state_dump.integration_dump.integration_limbo.len(), 0);
}

#[tokio::test(flavor = "multi_thread")]
async fn check_or_run_zome_init_does_nothing_if_already_initialized() {
    let zome = TestWasm::InitPass;
    let (dna, _, _) = SweetDnaFile::unique_from_test_wasms(vec![zome]).await;
    let mut conductor = SweetConductor::standard().await;
    let app = conductor.setup_app("app", [&dna]).await.unwrap();
    let cell_id = app.cells()[0].cell_id();
    let sweet_zome = app.cells()[0].zome(TestWasm::InitPass);

    // Trigger init.
    let _: InitCallbackResult = conductor.call(&sweet_zome, "init", ()).await;

    // Wait for integration workflow to complete
    retry_fn_until_timeout(
        || async { conductor.all_ops_integrated(dna.dna_hash()).unwrap() },
        None,
        None,
    )
    .await
    .unwrap();

    // Take state dump before calling check_or_run_zome_init
    let before_state_dump = conductor
        .raw_handle()
        .dump_full_cell_state(cell_id, None)
        .await
        .expect("Failed to get state dump");

    // Init zomes, so that all genesis records are committed
    let cell = conductor
        .raw_handle()
        .cell_by_id(cell_id)
        .await
        .expect("failed to get cell");
    cell.check_or_run_zome_init()
        .await
        .expect("failed to init cell");

    // Wait for integration workflow to complete
    retry_fn_until_timeout(
        || async { conductor.all_ops_integrated(dna.dna_hash()).unwrap() },
        None,
        None,
    )
    .await
    .unwrap();

    // Take state dump after calling check_or_run_zome_init
    let after_state_dump = conductor
        .raw_handle()
        .dump_full_cell_state(cell_id, None)
        .await
        .expect("Failed to get state dump");

    // DhtOps integrated have not changed, because initialization was already run
    assert_eq!(
        after_state_dump.integration_dump.integrated.len()
            - before_state_dump.integration_dump.integrated.len(),
        0
    );
}
