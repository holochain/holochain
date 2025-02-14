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
use kitsune2_api::{DhtArc, OpId, OpStore};
use std::sync::Arc;

fn test_dht_op(authored_timestamp: Timestamp) -> DhtOpHashed {
    let mut create = fixt!(Create);
    create.timestamp = authored_timestamp;

    let op = DhtOp::from(ChainOp::StoreRecord(
        fixt!(Signature),
        Action::Create(create),
        RecordEntry::Present(fixt!(Entry)),
    ));
    DhtOpHashed::from_content_sync(op)
}

#[tokio::test]
async fn process_incoming_ops_and_retrieve() {
    let db = DbWrite::test_in_mem(DbKindDht(Arc::new(DnaHash::from_raw_36(vec![0; 36])))).unwrap();
    let op_store = HolochainOpStore::new(db.clone());

    let dht_op_1 = test_dht_op(Timestamp::now());
    let dht_op_2 = test_dht_op(Timestamp::now());

    op_store
        .process_incoming_ops(vec![
            Bytes::from(holochain_serialized_bytes::encode(dht_op_1.as_content()).unwrap()),
            Bytes::from(holochain_serialized_bytes::encode(dht_op_2.as_content()).unwrap()),
        ])
        .await
        .unwrap();

    let to_retrieve = vec![
        OpId::from(Bytes::copy_from_slice(dht_op_1.as_hash().get_raw_36())),
        OpId::from(Bytes::copy_from_slice(dht_op_2.as_hash().get_raw_36())),
    ];

    // Ops are not integrated, we shouldn't be able to retrieve them
    let retrieved = op_store.retrieve_ops(to_retrieve.clone()).await.unwrap();
    assert!(retrieved.is_empty());

    set_all_integrated(db.clone()).await;

    // Ops are integrated, we should be able to retrieve them
    let retrieved = op_store.retrieve_ops(to_retrieve).await.unwrap();

    assert_eq!(2, retrieved.len());
}

#[tokio::test]
async fn retrieve_in_time_slice() {
    let db = DbWrite::test_in_mem(DbKindDht(Arc::new(DnaHash::from_raw_36(vec![0; 36])))).unwrap();
    let op_store = HolochainOpStore::new(db.clone());

    let mut dht_ops = Vec::with_capacity(5);
    for i in 0..5 {
        dht_ops.push(test_dht_op(Timestamp::from_micros((i + 1) * 100)))
    }

    let encoded_ops = dht_ops
        .iter()
        .map(|dht_op| Bytes::from(holochain_serialized_bytes::encode(dht_op.as_content()).unwrap()))
        .collect::<Vec<_>>();
    op_store
        .process_incoming_ops(encoded_ops.clone())
        .await
        .unwrap();

    let (hashes, size) = op_store
        .retrieve_op_hashes_in_time_slice(
            DhtArc::FULL,
            kitsune2_api::Timestamp::from_micros(0),
            kitsune2_api::Timestamp::now(),
        )
        .await
        .unwrap();
    assert!(hashes.is_empty());
    assert_eq!(0, size);

    set_all_integrated(db.clone()).await;

    // Get everything
    let (hashes, size) = op_store
        .retrieve_op_hashes_in_time_slice(
            DhtArc::FULL,
            kitsune2_api::Timestamp::from_micros(0),
            kitsune2_api::Timestamp::now(),
        )
        .await
        .unwrap();
    assert_eq!(5, hashes.len());
    assert_eq!(
        encoded_ops.iter().map(|b| b.len()).sum::<usize>() as u32,
        size
    );

    // Get just some ops by restricting the time slice
    let (hashes, size) = op_store
        .retrieve_op_hashes_in_time_slice(
            DhtArc::FULL,
            kitsune2_api::Timestamp::from_micros(0),
            kitsune2_api::Timestamp::from_micros(300),
        )
        .await
        .unwrap();
    assert_eq!(2, hashes.len());
    assert_eq!(
        encoded_ops.iter().take(2).map(|b| b.len()).sum::<usize>() as u32,
        size
    );

    // Get nothing because the time slice doesn't cover an ops
    let (hashes, size) = op_store
        .retrieve_op_hashes_in_time_slice(
            DhtArc::FULL,
            kitsune2_api::Timestamp::from_micros(1000),
            kitsune2_api::Timestamp::now(),
        )
        .await
        .unwrap();
    assert!(hashes.is_empty());
    assert_eq!(0, size);
}

#[tokio::test]
async fn retrieve_op_ids_bounded() {
    let db = DbWrite::test_in_mem(DbKindDht(Arc::new(DnaHash::from_raw_36(vec![0; 36])))).unwrap();
    let op_store = HolochainOpStore::new(db.clone());

    let mut dht_ops = Vec::with_capacity(1500);
    for i in 0..1500 {
        dht_ops.push(test_dht_op(Timestamp::from_micros((i + 1) * 100)))
    }

    let encoded_ops = dht_ops
        .iter()
        .map(|dht_op| Bytes::from(holochain_serialized_bytes::encode(dht_op.as_content()).unwrap()))
        .collect::<Vec<_>>();
    op_store
        .process_incoming_ops(encoded_ops.clone())
        .await
        .unwrap();

    // Set bounds to retrieve everything while ops are not integrated.
    let (hashes, size, timestamp) = op_store
        .retrieve_op_ids_bounded(
            DhtArc::FULL,
            kitsune2_api::Timestamp::from_micros(0),
            u32::MAX,
        )
        .await
        .unwrap();
    assert!(hashes.is_empty());
    assert_eq!(0, size);
    assert_eq!(timestamp, kitsune2_api::Timestamp::from_micros(0));

    set_all_integrated(db.clone()).await;

    // Get everything
    let (hashes, size, timestamp) = op_store
        .retrieve_op_ids_bounded(
            DhtArc::FULL,
            kitsune2_api::Timestamp::from_micros(0),
            u32::MAX,
        )
        .await
        .unwrap();
    assert_eq!(1500, hashes.len());
    assert_eq!(
        encoded_ops.iter().map(|b| b.len()).sum::<usize>() as u32,
        size
    );
    assert_eq!(kitsune2_api::Timestamp::from_micros(150000), timestamp);

    // Retrieve, bounded by the size of the first 750 ops
    let bounded_size = encoded_ops.iter().take(750).map(|b| b.len()).sum::<usize>() as u32;
    let (hashes, size, timestamp) = op_store
        .retrieve_op_ids_bounded(
            DhtArc::FULL,
            kitsune2_api::Timestamp::from_micros(0),
            bounded_size,
        )
        .await
        .unwrap();
    assert_eq!(750, hashes.len());
    assert_eq!(bounded_size, size);
    assert_eq!(kitsune2_api::Timestamp::from_micros(75000), timestamp);
}

#[tokio::test]
async fn create_and_read_slice_hashes() {
    let db = DbWrite::test_in_mem(DbKindDht(Arc::new(DnaHash::from_raw_36(vec![0; 36])))).unwrap();
    let op_store = HolochainOpStore::new(db.clone());

    op_store
        .store_slice_hash(DhtArc::Arc(0, 100), 5, Bytes::from_static(b"hash"))
        .await
        .unwrap();

    // Get the stored slice hash back
    let hash = op_store
        .retrieve_slice_hash(DhtArc::Arc(0, 100), 5)
        .await
        .unwrap();
    assert_eq!(Some(Bytes::from_static(b"hash")), hash);

    // Get all
    let hashes = op_store
        .retrieve_slice_hashes(DhtArc::Arc(0, 100))
        .await
        .unwrap();
    assert_eq!(1, hashes.len());
    assert_eq!(5, hashes[0].0);
    assert_eq!(Bytes::from_static(b"hash"), hashes[0].1);
}

#[tokio::test]
async fn count_slice_hashes() {
    let db = DbWrite::test_in_mem(DbKindDht(Arc::new(DnaHash::from_raw_36(vec![0; 36])))).unwrap();
    let op_store = HolochainOpStore::new(db.clone());

    op_store
        .store_slice_hash(DhtArc::Arc(0, 100), 5, Bytes::from_static(b"hash-a"))
        .await
        .unwrap();

    op_store
        .store_slice_hash(DhtArc::Arc(0, 100), 6, Bytes::from_static(b"hash-b"))
        .await
        .unwrap();

    op_store
        .store_slice_hash(DhtArc::Arc(0, 101), 5, Bytes::from_static(b"hash-c"))
        .await
        .unwrap();

    let count = op_store
        .slice_hash_count(DhtArc::Arc(0, 100))
        .await
        .unwrap();
    assert_eq!(2, count);

    let count = op_store
        .slice_hash_count(DhtArc::Arc(0, 101))
        .await
        .unwrap();
    assert_eq!(1, count);
}

#[tokio::test]
async fn slice_hashes_separate_by_arc() {
    let db = DbWrite::test_in_mem(DbKindDht(Arc::new(DnaHash::from_raw_36(vec![0; 36])))).unwrap();
    let op_store = HolochainOpStore::new(db.clone());

    op_store
        .store_slice_hash(DhtArc::Arc(0, 100), 5, Bytes::from_static(b"hash"))
        .await
        .unwrap();

    // Get a different arc
    let hashes = op_store
        .retrieve_slice_hashes(DhtArc::Arc(0, 101))
        .await
        .unwrap();
    assert!(hashes.is_empty());

    // Now store something in the other arc
    op_store
        .store_slice_hash(DhtArc::Arc(0, 101), 5, Bytes::from_static(b"hash-other"))
        .await
        .unwrap();

    // Both should be present
    let hashes = op_store
        .retrieve_slice_hashes(DhtArc::Arc(0, 100))
        .await
        .unwrap();
    assert_eq!(1, hashes.len());
    assert_eq!(5, hashes[0].0);
    assert_eq!(Bytes::from_static(b"hash"), hashes[0].1);

    let hashes = op_store
        .retrieve_slice_hashes(DhtArc::Arc(0, 101))
        .await
        .unwrap();
    assert_eq!(1, hashes.len());
    assert_eq!(5, hashes[0].0);
    assert_eq!(Bytes::from_static(b"hash-other"), hashes[0].1);
}

#[tokio::test]
async fn overwrite_slice_hashe() {
    let db = DbWrite::test_in_mem(DbKindDht(Arc::new(DnaHash::from_raw_36(vec![0; 36])))).unwrap();
    let op_store = HolochainOpStore::new(db.clone());

    op_store
        .store_slice_hash(DhtArc::Arc(0, 100), 5, Bytes::from_static(b"hash-a"))
        .await
        .unwrap();

    op_store
        .store_slice_hash(DhtArc::Arc(0, 100), 5, Bytes::from_static(b"hash-b"))
        .await
        .unwrap();

    let count = op_store
        .slice_hash_count(DhtArc::Arc(0, 100))
        .await
        .unwrap();
    assert_eq!(1, count);

    let hash = op_store
        .retrieve_slice_hash(DhtArc::Arc(0, 100), 5)
        .await
        .unwrap();
    assert_eq!(Some(Bytes::from_static(b"hash-b")), hash);
}

async fn set_all_integrated(db: DbWrite<DbKindDht>) {
    db.write_async(move |txn| -> StateMutationResult<()> {
        txn.execute("UPDATE DhtOp SET when_integrated = authored_timestamp", [])?;

        Ok(())
    })
    .await
    .unwrap();
}
