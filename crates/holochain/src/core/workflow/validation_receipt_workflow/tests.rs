use crate::test_utils::consistency_10s;
use crate::test_utils::sweetest::*;
use fallible_iterator::FallibleIterator;
use hdk::prelude::*;
use holo_hash::DhtOpHash;
use holochain_keystore::AgentPubKeyExt;
use holochain_lmdb::buffer::KvBufFresh;
use holochain_lmdb::db::GetDb;
use holochain_lmdb::db::AUTHORED_DHT_OPS;
use holochain_lmdb::env::EnvironmentRead;
use holochain_lmdb::fresh_reader_test;
use holochain_state::prelude::*;
use holochain_types::dht_op::produce_ops_from_element;
use holochain_types::dna::zome::inline_zome::InlineZome;

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
    let env: EnvironmentRead = alice.env().clone().into();
    let sc = SourceChain::new(env.clone()).unwrap();
    let element = sc.get_element(&hash).unwrap().unwrap();
    let ops = produce_ops_from_element(&element)
        .unwrap()
        .into_iter()
        .map(|op| DhtOpHash::with_data_sync(&op))
        .collect::<Vec<_>>();

    // Wait for receipts to be sent
    let db = ValidationReceiptsBuf::new(&env).unwrap();

    crate::assert_eq_retry_10s!(
        {
            let mut counts = Vec::new();
            for hash in &ops {
                let count = fresh_reader_test!(env, |r| db
                    .list_receipts(&r, hash)
                    .unwrap()
                    .count()
                    .unwrap());
                counts.push(count);
            }
            counts
        },
        vec![2, 2, 2],
    );

    // Check alice has receipts from both bobbo and carol
    for hash in ops {
        let receipts: Vec<_> = fresh_reader_test!(env, |r| db
            .list_receipts(&r, &hash)
            .unwrap()
            .collect()
            .unwrap());
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
    let db = env.get_db(&*AUTHORED_DHT_OPS).unwrap();
    let authored_dht_ops: AuthoredDhtOpsStore = KvBufFresh::new(env.clone(), db);
    crate::assert_eq_retry_10s!(
        {
            fresh_reader_test!(env, |mut r| authored_dht_ops
                .iter(&mut r)
                .unwrap()
                .map(|(_, v)| Ok(v.receipt_count))
                .collect::<Vec<_>>()
                .unwrap())
        },
        vec![2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2]
    );
}
