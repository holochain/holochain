use crate::sweettest::*;
use crate::test_utils::consistency_10s;
use crate::test_utils::inline_zomes::simple_create_read_zome;
use hdk::prelude::*;
use holo_hash::DhtOpHash;
use holochain_keystore::AgentPubKeyExt;
use holochain_sqlite::prelude::*;
use holochain_state::prelude::*;
use holochain_types::prelude::*;
use rusqlite::Transaction;

#[tokio::test(flavor = "multi_thread")]
#[ignore = "flaky"]
async fn test_validation_receipt() {
    let _g = observability::test_run().ok();
    const NUM_CONDUCTORS: usize = 3;

    let mut conductors = SweetConductorBatch::from_standard_config(NUM_CONDUCTORS).await;

    let (dna_file, _, _) =
        SweetDnaFile::unique_from_inline_zomes(("simple", simple_create_read_zome()))
            .await
            .unwrap();

    let apps = conductors.setup_app("app", &[dna_file]).await.unwrap();
    conductors.exchange_peer_info().await;

    let ((alice,), (bobbo,), (carol,)) = apps.into_tuples();

    // Call the "create" zome fn on Alice's app
    let hash: ActionHash = conductors[0]
        .call(&alice.zome("simple"), "create", ())
        .await;

    consistency_10s(&[&alice, &bobbo, &carol]).await;

    // Get op hashes
    let vault = alice.dht_db().clone().into();
    let record = fresh_store_test(&vault, |store| {
        store.get_record(&hash.clone().into()).unwrap().unwrap()
    });
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
                let count = fresh_reader_test!(vault, |r| list_receipts(&r, hash).unwrap().len());
                counts.push(count);
            }
            counts
        },
        vec![2, 2, 2],
    );

    // Check alice has receipts from both bobbo and carol
    for hash in ops {
        let receipts: Vec<_> =
            fresh_reader_test!(vault, |mut r| list_receipts(&mut r, &hash).unwrap());
        assert_eq!(receipts.len(), 2);
        for receipt in receipts {
            let SignedValidationReceipt {
                receipt,
                validators_signatures: sigs,
            } = receipt;
            let validator = receipt.validators[0].clone();
            assert!(validator == *bobbo.agent_pubkey() || validator == *carol.agent_pubkey());
            assert!(validator.verify_signature(&sigs[0], receipt).await);
        }
    }

    // Check alice has 2 receipts in their authored dht ops table.
    crate::assert_eq_retry_1m!(
        {
            fresh_reader_test!(vault, |txn: Transaction| {
                let mut stmt = txn
                    .prepare("SELECT COUNT(hash) FROM ValidationReceipt GROUP BY op_hash")
                    .unwrap();
                stmt.query_map([], |row| row.get::<_, Option<u32>>(0))
                    .unwrap()
                    .map(Result::unwrap)
                    .filter_map(|i| i)
                    .collect::<Vec<u32>>()
            })
        },
        vec![2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2]
    );
}
