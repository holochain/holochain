use crate::conductor::api::error::ConductorApiError;
use crate::conductor::CellError;
use crate::conductor::ConductorHandle;
use crate::core::workflow::error::WorkflowError;
use crate::core::SourceChainError;
use crate::test_utils::new_zome_call;
use crate::test_utils::setup_app;
use holochain_serialized_bytes::SerializedBytes;
use holochain_types::prelude::*;
use holochain_wasm_test_utils::TestWasm;
use holochain_zome_types::cell::CellId;
use std::convert::TryFrom;

#[tokio::test(threaded_scheduler)]
async fn direct_validation_test() {
    observability::test_run().ok();

    let dna_file = DnaFile::new(
        DnaDef {
            name: "direct_validation_test".to_string(),
            uuid: "ba1d046d-ce29-4778-914b-47e6010d2faf".to_string(),
            properties: SerializedBytes::try_from(()).unwrap(),
            zomes: vec![TestWasm::Update.into()].into(),
        },
        vec![TestWasm::Update.into()],
    )
    .await
    .unwrap();

    let alice_agent_id = fake_agent_pubkey_1();
    let alice_cell_id = CellId::new(dna_file.dna_hash().to_owned(), alice_agent_id.clone());
    let alice_installed_cell = InstalledCell::new(alice_cell_id.clone(), "alice_handle".into());

    let (_tmpdir, _app_api, handle) = setup_app(
        vec![("test_app", vec![(alice_installed_cell, None)])],
        vec![dna_file.clone()],
    )
    .await;

    run_test(alice_cell_id, handle.clone()).await;

    let shutdown = handle.take_shutdown_handle().await.unwrap();
    handle.shutdown().await;
    shutdown.await.unwrap();
}

/// - Commit a valid update should pass
/// - Commit an invalid update should fail the zome call
async fn run_test(alice_cell_id: CellId, handle: ConductorHandle) {
    // Valid update should work
    let invocation = new_zome_call(&alice_cell_id, "update_entry", (), TestWasm::Update).unwrap();
    handle.call_zome(invocation).await.unwrap().unwrap();

    // Invalid update should fail work
    let invocation =
        new_zome_call(&alice_cell_id, "invalid_update_entry", (), TestWasm::Update).unwrap();
    let result = handle.call_zome(invocation).await;
    match &result {
        Err(ConductorApiError::CellError(CellError::WorkflowError(wfe))) => match **wfe {
            WorkflowError::SourceChainError(SourceChainError::InvalidCommit(_)) => {}
            _ => panic!("Expected InvalidCommit got {:?}", result),
        },
        _ => panic!("Expected InvalidCommit got {:?}", result),
    }
}
// ,
