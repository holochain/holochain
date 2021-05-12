use crate::sweettest::*;
use crate::test_utils::consistency_10s;
use hdk::prelude::*;
use holo_hash::DhtOpHash;
use holochain_keystore::AgentPubKeyExt;
use holochain_sqlite::prelude::*;
use holochain_state::prelude::*;
use holochain_types::dna::zome::inline_zome::InlineZome;
use holochain_types::{dht_op::produce_ops_from_element, env::EnvRead};
use rusqlite::Transaction;

fn simple_crud_zome() -> InlineZome {
    let entry_def = EntryDef::default_with_id("entrydef");

    InlineZome::new_unique(vec![entry_def.clone()])
        .callback("create", move |api, ()| {
            let entry_def_id: EntryDefId = entry_def.id.clone();
            let entry = Entry::app(().try_into().unwrap()).unwrap();
            let hash = api.create(EntryWithDefId::new(entry_def_id, entry))?;
            Ok(hash)
        })
        .callback("read", |api, hash: HeaderHash| {
            api.get(GetInput::new(hash.into(), GetOptions::default()))
                .map_err(Into::into)
        })
}

#[tokio::test(flavor = "multi_thread")]
async fn test_validation_receipt() {
    let _g = observability::test_run().ok();
    const NUM_CONDUCTORS: usize = 3;

    let mut conductors = SweetConductorBatch::from_standard_config(NUM_CONDUCTORS).await;

    let (dna_file, _) = SweetDnaFile::unique_from_inline_zome("zome1", simple_crud_zome())
        .await
        .unwrap();

    let apps = conductors.setup_app("app", &[dna_file]).await.unwrap();
    conductors.exchange_peer_info().await;

    let ((alice,), (bobbo,), (carol,)) = apps.into_tuples();

    // Call the "create" zome fn on Alice's app
    let hash: HeaderHash = conductors[0].call(&alice.zome("zome1"), "create", ()).await;

    consistency_10s(&[&alice, &bobbo, &carol]).await;

    // Get op hashes
    let vault: EnvRead = alice.env().clone().into();
    let element = fresh_store_test(&vault, |store| {
        store.get_element(&hash.clone().into()).unwrap().unwrap()
    });
    let ops = produce_ops_from_element(&element)
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
                validator_signature: sig,
            } = receipt;
            let validator = receipt.validator.clone();
            assert!(validator == *bobbo.agent_pubkey() || validator == *carol.agent_pubkey());
            assert!(validator.verify_signature(&sig, receipt).await.unwrap());
        }
    }

    // Check alice has 2 receipts in their authored dht ops table.
    crate::assert_eq_retry_1m!(
        {
            fresh_reader_test!(vault, |txn: Transaction| {
                let mut stmt = txn
                    .prepare("SELECT receipt_count FROM DhtOp WHERE is_authored = 1")
                    .unwrap();
                stmt.query_map([], |row| row.get::<_, Option<u32>>("receipt_count"))
                    .unwrap()
                    .map(Result::unwrap)
                    .filter_map(|i| i)
                    .collect::<Vec<u32>>()
            })
        },
        vec![2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2]
    );
}
