use bytes::Bytes;
use fixt::fixt;
use holo_hash::{DnaHash, HasHash};
use holochain_p2p::HolochainOpStore;
use holochain_sqlite::db::{DbKindDht, DbWrite};
use holochain_state::prelude::{RecordEntry, StateMutationResult};
use holochain_timestamp::Timestamp;
use holochain_types::dht_op::{ChainOp, DhtOpHashed};
use holochain_types::prelude::DhtOp;
use holochain_zome_types::fixt::{CreateFixturator, EntryFixturator, SignatureFixturator};
use holochain_zome_types::Action;
use kitsune2_api::{OpId, OpStore};
use std::sync::Arc;

fn test_dht_op() -> DhtOpHashed {
    let op = DhtOp::from(ChainOp::StoreRecord(
        fixt!(Signature),
        Action::Create(fixt!(Create)),
        RecordEntry::Present(fixt!(Entry)),
    ));
    DhtOpHashed::from_content_sync(op)
}

#[tokio::test]
async fn process_incoming_ops_and_retrieve() {
    let db = DbWrite::test_in_mem(DbKindDht(Arc::new(DnaHash::from_raw_36(vec![0; 36])))).unwrap();
    let op_store = HolochainOpStore::new(db.clone());

    let dht_op_1 = test_dht_op();
    let dht_op_2 = test_dht_op();

    op_store
        .process_incoming_ops(vec![
            Bytes::from(holochain_serialized_bytes::encode(dht_op_1.as_content()).unwrap()),
            Bytes::from(holochain_serialized_bytes::encode(dht_op_2.as_content()).unwrap()),
        ])
        .await
        .unwrap();

    db.write_async(move |txn| -> StateMutationResult<()> {
        txn.execute(
            "UPDATE DhtOp SET when_integrated = ?",
            [Timestamp::now().as_millis()],
        )?;

        Ok(())
    })
    .await
    .unwrap();

    let to_retrieve = vec![
        OpId::from(Bytes::copy_from_slice(dht_op_1.as_hash().get_raw_36())),
        OpId::from(Bytes::copy_from_slice(dht_op_2.as_hash().get_raw_36())),
    ];
    println!("{:?}", to_retrieve);
    let retrieved = op_store.retrieve_ops(to_retrieve).await.unwrap();

    assert_eq!(2, retrieved.len());
}
