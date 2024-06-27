use holo_hash::ActionHash;
use holochain::core::workflow::publish_dht_ops_workflow::num_still_needing_publish;
use holochain::sweettest::*;
use holochain_wasm_test_utils::TestWasm;
use holochain_zome_types::prelude::GetValidationReceiptsInput;
use holochain_zome_types::validate::ValidationReceiptSet;

/// Verifies that publishing terminates naturally when enough validation receipts are received.
#[cfg(feature = "test_utils")]
#[tokio::test(flavor = "multi_thread")]
async fn publish_terminates_after_receiving_required_validation_receipts() {
    holochain_trace::test_run();

    // Need DEFAULT_RECEIPT_BUNDLE_SIZE peers to send validation receipts back
    const NUM_CONDUCTORS: usize =
        holochain::core::workflow::publish_dht_ops_workflow::DEFAULT_RECEIPT_BUNDLE_SIZE as usize
            + 1;

    let mut conductors = SweetConductorBatch::from_config_rendezvous(
        NUM_CONDUCTORS,
        SweetConductorConfig::rendezvous(true),
    )
    .await;

    let (dna_file, _, _) = SweetDnaFile::unique_from_test_wasms(vec![TestWasm::Create]).await;

    let apps = conductors.setup_app("app", &[dna_file]).await.unwrap();

    let ((alice,), (bobbo,), (carol,), (danny,), (emma,), (fred,)) = apps.into_tuples();

    let action_hash: ActionHash = conductors[0]
        .call(&alice.zome(TestWasm::Create), "create_entry", ())
        .await;

    // Wait until they all see the created entry, at that point validation receipts should be getting sent soon
    await_consistency(60, [&alice, &bobbo, &carol, &danny, &emma, &fred])
        .await
        .unwrap();

    let ops_to_publish = alice
        .authored_db()
        .read_async({
            let alice_pub_key = alice.agent_pubkey().clone();
            // Note that this test is relying on this being the same check that the publish workflow uses.
            // If this returns 0 then the publish workflow is expected to suspend. So the test isn't directly
            // observing that behaviour but it's close enough given that there are unit tests for the actual
            // behavior.
            move |txn| num_still_needing_publish(&txn, alice_pub_key)
        })
        .await
        .unwrap();

    // Get the validation receipts to check that they are all complete
    let receipt_sets: Vec<ValidationReceiptSet> = conductors[0]
        .call(
            &alice.zome(TestWasm::Create),
            "get_validation_receipts",
            GetValidationReceiptsInput::for_action(action_hash),
        )
        .await;
    assert_eq!(receipt_sets.len(), 3);
    assert!(receipt_sets.iter().all(|r| r.receipts_complete));

    let agent_activity_receipt_set = receipt_sets
        .into_iter()
        .find(|r| r.op_type == "RegisterAgentActivity")
        .unwrap();
    assert_eq!(
        agent_activity_receipt_set.receipts.len(),
        holochain::core::workflow::publish_dht_ops_workflow::DEFAULT_RECEIPT_BUNDLE_SIZE as usize
    );

    assert_eq!(ops_to_publish, 0);
}
