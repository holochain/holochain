use holo_hash::ActionHash;
use holochain::sweettest::*;
use holochain_wasm_test_utils::TestWasm;
use holochain_zome_types::prelude::GetValidationReceiptsInput;
use holochain_zome_types::validate::ValidationReceiptSet;

/// Verifies that publishing terminates naturally when enough validation receipts are received.
#[cfg(feature = "test_utils")]
#[tokio::test(flavor = "multi_thread")]
#[cfg_attr(
    not(any(target_os = "linux", all(target_os = "macos", feature = "wasmer_sys"))),
    ignore = "flaky on macos+wasmer_wamr and windows"
)]
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

    let apps = [alice, bobbo, carol, danny, emma, fred];

    for c in conductors.iter() {
        c.declare_full_storage_arcs(apps[0].dna_hash()).await;
    }

    // wait for all our conductors to see each other
    tokio::time::timeout(std::time::Duration::from_secs(60), async {
        loop {
            let mut all_good = true;

            for c in conductors.iter() {
                if c.holochain_p2p()
                    .peer_store(apps[0].dna_hash().clone())
                    .await
                    .unwrap()
                    .get_all()
                    .await
                    .unwrap()
                    .into_iter()
                    .count()
                    < 6
                {
                    all_good = false;
                    break;
                }
            }

            if all_good {
                break;
            }

            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();

    // write an action
    let action_hash: ActionHash = conductors[0]
        .call(&apps[0].zome(TestWasm::Create), "create_entry", ())
        .await;

    // wait for validation receipts
    tokio::time::timeout(std::time::Duration::from_secs(60), async {
        loop {
            // check for complete count of our receipts on the
            // millisecond level

            // Get the validation receipts to check that they
            // are all complete
            let receipt_sets: Vec<ValidationReceiptSet> = conductors[0]
                .call(
                    &apps[0].zome(TestWasm::Create),
                    "get_validation_receipts",
                    GetValidationReceiptsInput::new(action_hash.clone()),
                )
                .await;

            let receipt_sets_len = receipt_sets.len() == 3;
            let receipt_sets_complete = receipt_sets.iter().all(|r| r.receipts_complete);
            let agent_activity_receipt_set = match receipt_sets
                .into_iter()
                .find(|r| r.op_type == "RegisterAgentActivity")
            {
                None => 0,
                Some(r) => r.receipts.len(),
            }
                == holochain::core::workflow::publish_dht_ops_workflow::DEFAULT_RECEIPT_BUNDLE_SIZE
                    as usize;

            if receipt_sets_len && receipt_sets_complete && agent_activity_receipt_set {
                // Test Passed!
                return;
            }

            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
}
