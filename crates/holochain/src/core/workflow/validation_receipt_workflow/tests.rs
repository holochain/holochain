use crate::sweettest::*;
use crate::test_utils::inline_zomes::simple_create_read_zome;
use hdk::prelude::*;
use holo_hash::DhtOpHash;
use holochain_keystore::AgentPubKeyExt;
use holochain_state::prelude::*;

#[tokio::test(flavor = "multi_thread")]
#[ignore = "flaky, doesn't take into account timing or retries"]
async fn test_validation_receipt() {
    holochain_trace::test_run();
    const NUM_CONDUCTORS: usize = 3;

    let mut conductors = SweetConductorBatch::from_standard_config(NUM_CONDUCTORS).await;

    let (dna_file, _, _) =
        SweetDnaFile::unique_from_inline_zomes(("simple", simple_create_read_zome())).await;

    let apps = conductors.setup_app("app", &[dna_file]).await.unwrap();
    conductors.exchange_peer_info().await;

    let ((alice,), (bobbo,), (carol,)) = apps.into_tuples();

    // Call the "create" zome fn on Alice's app
    let hash: ActionHash = conductors[0]
        .call(&alice.zome("simple"), "create", ())
        .await;

    await_consistency([&alice, &bobbo, &carol]).await.unwrap();

    // Get op hashes
    let vault = alice.dht_db();
    let record = vault
        .read_async(move |txn| -> StateQueryResult<Record> {
            Ok(CascadeTxnWrapper::from(txn)
                .get_record(&hash.clone().into())?
                .unwrap())
        })
        .await
        .unwrap();
    let ops = produce_ops_from_record(&record)
        .unwrap()
        .into_iter()
        .map(|op| DhtOpHash::with_data_sync(&op))
        .collect::<Vec<_>>();

    // Wait for receipts to be sent
    crate::assert_eq_retry_10s!(
        {
            let mut counts = Vec::new();
            for hash in &ops {
                let count = vault
                    .read_async({
                        let query_hash = hash.clone();
                        move |txn| -> StateQueryResult<usize> {
                            Ok(list_receipts(txn, &query_hash)?.len())
                        }
                    })
                    .await
                    .unwrap();
                counts.push(count);
            }
            counts
        },
        vec![2, 2, 2],
    );

    // Check alice has receipts from both bobbo and carol
    for hash in &ops {
        let receipts: Vec<_> = vault
            .read_async({
                let query_hash = hash.clone();
                move |txn| list_receipts(txn, &query_hash)
            })
            .await
            .unwrap();
        assert_eq!(receipts.len(), 2);
        for receipt in receipts {
            let SignedValidationReceipt {
                receipt,
                validators_signatures: sigs,
            } = receipt;
            let validator = receipt.validators[0].clone();
            assert!(validator == *bobbo.agent_pubkey() || validator == *carol.agent_pubkey());
            assert!(validator.verify_signature(&sigs[0], receipt).await.unwrap());
        }
    }

    // Check alice has 2 receipts in their authored dht ops table.
    crate::assert_eq_retry_1m!(
        {
            vault
                .read_async(move |txn| -> DatabaseResult<Vec<u32>> {
                    let mut stmt = txn
                        .prepare("SELECT COUNT(hash) FROM ValidationReceipt GROUP BY op_hash")
                        .unwrap();
                    Ok(stmt
                        .query_map([], |row| row.get::<_, Option<u32>>(0))
                        .unwrap()
                        .filter_map(Result::unwrap)
                        .collect::<Vec<u32>>())
                })
                .await
                .unwrap()
        },
        vec![2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2]
    );
}
