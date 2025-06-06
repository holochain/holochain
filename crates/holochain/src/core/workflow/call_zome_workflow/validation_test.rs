use crate::conductor::api::error::ConductorApiError;
use crate::conductor::CellError;
use crate::conductor::ConductorHandle;
use crate::core::workflow::WorkflowError;
use crate::core::SourceChainError;
use crate::test_utils::new_zome_call_params;
use crate::test_utils::setup_app_with_names;
use holochain_serialized_bytes::SerializedBytes;
use holochain_types::prelude::*;
use holochain_wasm_test_utils::TestWasm;
use holochain_wasm_test_utils::TestWasmPair;
use holochain_wasm_test_utils::TestZomes;
use holochain_zome_types::cell::CellId;
use std::convert::TryFrom;

#[tokio::test(flavor = "multi_thread")]
async fn direct_validation_test() {
    holochain_trace::test_run();

    let TestWasmPair::<DnaWasm> {
        integrity,
        coordinator,
    } = TestWasm::Update.into();
    let dna_file = DnaFile::new(
        DnaDef {
            name: "direct_validation_test".to_string(),
            modifiers: DnaModifiers {
                network_seed: "ba1d046d-ce29-4778-914b-47e6010d2faf".to_string(),
                properties: SerializedBytes::try_from(()).unwrap(),
            },
            integrity_zomes: vec![TestZomes::from(TestWasm::Update).integrity.into_inner()],
            coordinator_zomes: vec![TestZomes::from(TestWasm::Update).coordinator.into_inner()],
            #[cfg(feature = "unstable-migration")]
            lineage: Default::default(),
        },
        [integrity, coordinator],
    )
    .await;

    let alice_agent_id = fake_agent_pubkey_1();
    let alice_cell_id = CellId::new(dna_file.dna_hash().to_owned(), alice_agent_id.clone());

    let (_tmpdir, _app_api, handle) =
        setup_app_with_names(alice_agent_id, vec![("test_app", vec![(dna_file, None)])]).await;

    run_test(alice_cell_id, handle.clone()).await;

    handle.shutdown().await.unwrap().unwrap();
}

// - Commit a valid update should pass
// - Commit an invalid update should fail the zome call
async fn run_test(alice_cell_id: CellId, handle: ConductorHandle) {
    // Valid update should work
    let zome_call_params =
        new_zome_call_params(&alice_cell_id, "update_entry", (), TestWasm::Update).unwrap();
    handle.call_zome(zome_call_params).await.unwrap().unwrap();

    // Valid update of a private entry should work
    let zome_call_params =
        new_zome_call_params(&alice_cell_id, "update_private_entry", (), TestWasm::Update).unwrap();
    handle.call_zome(zome_call_params).await.unwrap().unwrap();

    // Invalid update should fail work
    let zome_call_params =
        new_zome_call_params(&alice_cell_id, "invalid_update_entry", (), TestWasm::Update).unwrap();
    let result = handle.call_zome(zome_call_params).await;
    match &result {
        Err(ConductorApiError::CellError(CellError::WorkflowError(wfe))) => match **wfe {
            WorkflowError::SourceChainError(SourceChainError::InvalidCommit(_)) => {}
            _ => panic!("Expected InvalidCommit got {:?}", result),
        },
        _ => panic!("Expected InvalidCommit got {:?}", result),
    }
}
// ,
