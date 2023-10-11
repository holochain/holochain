use holo_hash::ActionHash;
use holochain::core::workflow::publish_dht_ops_workflow::num_still_needing_publish;
use holochain::sweettest::{
    consistency_60s, SweetConductorBatch, SweetConductorConfig, SweetDnaFile,
};
use holochain_wasm_test_utils::TestWasm;
use std::time::Duration;

/// Verifies that publishing terminates naturally when enough validation receipts are received.
#[cfg(feature = "test_utils")]
#[tokio::test(flavor = "multi_thread")]
async fn publish_termination() {
    let _g = holochain_trace::test_run().unwrap();
    const NUM_CONDUCTORS: usize = 6; // Need 5 peers to send validation receipts back

    let mut conductors = SweetConductorBatch::from_config_rendezvous(
        NUM_CONDUCTORS,
        SweetConductorConfig::rendezvous(),
    )
    .await;

    let (dna_file, _, _) = SweetDnaFile::unique_from_test_wasms(vec![TestWasm::Create]).await;

    let apps = conductors.setup_app("app", &[dna_file]).await.unwrap();

    let ((alice,), (bobbo,), (carol,), (danny,), (emma,), (fred,)) = apps.into_tuples();

    let _: ActionHash = conductors[0]
        .call(&alice.zome(TestWasm::Create), "create_entry", ())
        .await;

    // Wait until they all see the created entry, at that point validation receipts should be getting sent soon
    consistency_60s([&alice, &bobbo, &carol, &danny, &emma, &fred]).await;

    let ops_to_publish = tokio::time::timeout(Duration::from_secs(30), async {
        let alice_pub_key = alice.agent_pubkey().clone();
        loop {
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

            println!("Ops to publish {}", ops_to_publish);

            if ops_to_publish == 0 {
                return ops_to_publish;
            }

            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();

    assert_eq!(0, ops_to_publish);
}
