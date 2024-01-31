use holo_hash::ActionHash;
use holochain::core::workflow::publish_dht_ops_workflow::num_still_needing_publish;
use holochain::sweettest::{
    consistency_60s, SweetConductorBatch, SweetConductorConfig, SweetDnaFile,
};
use holochain_wasm_test_utils::TestWasm;
use std::time::Duration;
use holochain_sqlite::error::DatabaseResult;

/// Verifies that publishing terminates naturally when enough validation receipts are received.
#[cfg(feature = "test_utils")]
#[tokio::test(flavor = "multi_thread")]
#[cfg_attr(target_os = "macos", ignore = "flaky")]
async fn publish_termination() {
    use rusqlite::named_params;

    let _g = holochain_trace::test_run().unwrap();

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
    consistency_60s([&alice, &bobbo, &carol, &danny, &emma, &fred]).await;

    let ops_to_publish = tokio::time::timeout(Duration::from_secs(60), async {
        let alice_pub_key = alice.agent_pubkey().clone();
        loop {
            tokio::time::sleep(Duration::from_millis(100)).await;

            let ops_to_publish = alice
                .authored_db()
                .read_async({
                    let alice_pub_key = alice_pub_key.clone();
                    // Note that this test is relying on this being the same check that the publish workflow uses.
                    // If this returns 0 then the publish workflow is expected to suspend. So the test isn't directly
                    // observing that behaviour but it's close enough given that there are unit tests for the actual
                    // behavior.
                    move |txn| num_still_needing_publish(&txn, alice_pub_key)
                })
                .await
                .unwrap();

            if ops_to_publish == 0 {
                return ops_to_publish;
            }
        }
    })
    .await;

    use holochain_types::validation_receipt::{SignedValidationReceipt, ValidationReceipt, ValidationReceiptBundle};
    if !ops_to_publish.is_ok() {
        let receipt_count = alice.dht_db().read_async(move |txn| -> DatabaseResult<usize> {
            let stmt = txn.prepare("SELECT blob FROM ValidationReceipt INNER JOIN DhtOp ON DhtOp.hash = ValidationReceipt.op_hash WHERE DhtOp.action_hash = :action_hash").unwrap();
    
            let x: Vec<SignedValidationReceipt> = stmt.query(named_params! {
                ":action_hash": action_hash,
            }).unwrap().and_then(|r| r.get(0)).collect::<rusqlite::Result<Vec<SignedValidationReceipt>>>()?;

            Ok(x.len())
        }).await.unwrap();

        assert_eq!(5, receipt_count, "Expected 5 validation receipts to be received");
    }

    assert_eq!(Ok(0), ops_to_publish);
}
