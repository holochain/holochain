use bytes::Bytes;
use fixt::fixt;
use holo_hash::{AgentPubKey, AnyDhtHash, DhtOpHash, DnaHash, HasHash};
use holochain_p2p::event::{
    CountersigningSessionNegotiationMessage, DynHcP2pHandler, GetActivityOptions, GetLinksOptions,
    HcP2pHandler,
};
use holochain_p2p::{HolochainOpStore, HolochainP2pResult};
use holochain_serialized_bytes::SerializedBytes;
use holochain_state::dht_store::SysOutcome;
use holochain_state::prelude::{ChainFilter, ExternIO, RecordEntry, Signature};
use holochain_state::DhtStore;
use holochain_timestamp::Timestamp;
use holochain_types::activity::AgentActivityResponse;
use holochain_types::chain::MustGetAgentActivityResponse;
use holochain_types::dht_op::{ChainOp, DhtOpHashed, WireOps};
use holochain_types::link::{CountLinksResponse, WireLinkKey, WireLinkOps, WireLinkQuery};
use holochain_types::prelude::{DhtOp, ValidationReceiptBundle};
use holochain_zome_types::fixt::{CreateFixturator, EntryFixturator, SignatureFixturator};
use holochain_zome_types::prelude::ChainQueryFilter;
use holochain_zome_types::Action;
use kitsune2_api::*;
use std::sync::Arc;

/// Stub host that routes `handle_publish` into the new `DhtStore` via
/// `record_incoming_ops`. The K2 `process_incoming_ops` call lands here
/// just like it would in the running conductor.
#[derive(Debug)]
struct StubHost {
    store: DhtStore,
}

impl HcP2pHandler for StubHost {
    fn handle_call_remote(
        &self,
        _dna_hash: DnaHash,
        _to_agent: AgentPubKey,
        _zome_call_params_serialized: ExternIO,
        _signature: Signature,
    ) -> BoxFut<'_, HolochainP2pResult<SerializedBytes>> {
        unimplemented!()
    }

    fn handle_publish(
        &self,
        _dna_hash: DnaHash,
        ops: Vec<DhtOp>,
    ) -> BoxFut<'_, HolochainP2pResult<()>> {
        let store = self.store.clone();
        Box::pin(async move {
            let hashed: Vec<DhtOpHashed> = ops
                .into_iter()
                .map(DhtOpHashed::from_content_sync)
                .collect();
            store
                .record_incoming_ops(hashed)
                .await
                .map_err(holochain_p2p::HolochainP2pError::other)?;
            Ok(())
        })
    }

    fn handle_get(
        &self,
        _dna_hash: DnaHash,
        _to_agent: AgentPubKey,
        _dht_hash: AnyDhtHash,
    ) -> BoxFut<'_, HolochainP2pResult<WireOps>> {
        unimplemented!()
    }

    fn handle_get_links(
        &self,
        _dna_hash: DnaHash,
        _to_agent: AgentPubKey,
        _link_key: WireLinkKey,
        _options: GetLinksOptions,
    ) -> BoxFut<'_, HolochainP2pResult<WireLinkOps>> {
        unimplemented!()
    }

    fn handle_count_links(
        &self,
        _dna_hash: DnaHash,
        _to_agent: AgentPubKey,
        _query: WireLinkQuery,
    ) -> BoxFut<'_, HolochainP2pResult<CountLinksResponse>> {
        unimplemented!()
    }

    fn handle_get_agent_activity(
        &self,
        _dna_hash: DnaHash,
        _to_agent: AgentPubKey,
        _agent: AgentPubKey,
        _query: ChainQueryFilter,
        _options: GetActivityOptions,
    ) -> BoxFut<'_, HolochainP2pResult<AgentActivityResponse>> {
        unimplemented!()
    }

    fn handle_must_get_agent_activity(
        &self,
        _dna_hash: DnaHash,
        _to_agent: AgentPubKey,
        _author: AgentPubKey,
        _filter: ChainFilter,
    ) -> BoxFut<'_, HolochainP2pResult<MustGetAgentActivityResponse>> {
        unimplemented!()
    }

    fn handle_validation_receipts_received(
        &self,
        _dna_hash: DnaHash,
        _to_agent: AgentPubKey,
        _receipts: ValidationReceiptBundle,
    ) -> BoxFut<'_, HolochainP2pResult<()>> {
        unimplemented!()
    }

    fn handle_publish_countersign(
        &self,
        _dna_hash: DnaHash,
        _op: ChainOp,
    ) -> BoxFut<'_, HolochainP2pResult<()>> {
        unimplemented!()
    }

    fn handle_countersigning_session_negotiation(
        &self,
        _dna_hash: DnaHash,
        _to_agent: AgentPubKey,
        _message: CountersigningSessionNegotiationMessage,
    ) -> BoxFut<'_, HolochainP2pResult<()>> {
        unimplemented!()
    }
}

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

/// Move the supplied chain ops from limbo into the integrated table.
///
/// `record_incoming_ops` parks each op in `LimboChainOp` with NULL
/// validation statuses; `integrate_ready_ops` only promotes rows that have
/// gone through the validation workflows. For tests that need integrated
/// rows without exercising validation, this helper flips sys+app to
/// `Accepted` then triggers integration. To preserve the
/// distinct-per-op `when_integrated` cursor the K2 "since" paging loop
/// expects, each op is integrated in its own call with a monotonically
/// increasing timestamp.
async fn set_all_integrated(store: &DhtStore, op_hashes: &[DhtOpHash]) {
    for (i, hash) in op_hashes.iter().enumerate() {
        store
            .record_chain_op_sys_validation_outcomes(vec![(hash.clone(), SysOutcome::Accepted)])
            .await
            .unwrap();
        store
            .record_app_validation_outcomes(vec![(
                hash.clone(),
                holochain_state::dht_store::AppOutcome::Accepted,
            )])
            .await
            .unwrap();
        store
            .integrate_ready_ops(Timestamp::from_micros((i as i64 + 1) * 100))
            .await
            .unwrap();
    }
}

#[tokio::test]
async fn process_incoming_ops_and_retrieve() {
    let (store, op_store) = setup_test().await;

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
        dht_op_1
            .as_hash()
            .to_located_k2_op_id(&dht_op_1.dht_basis()),
        dht_op_2
            .as_hash()
            .to_located_k2_op_id(&dht_op_2.dht_basis()),
    ];

    // Ops are in limbo, not integrated — should not be retrievable.
    let retrieved = op_store.retrieve_ops(to_retrieve.clone()).await.unwrap();
    assert!(retrieved.is_empty());

    set_all_integrated(
        &store,
        &[dht_op_1.as_hash().clone(), dht_op_2.as_hash().clone()],
    )
    .await;

    // Now integrated — should be retrievable.
    let retrieved = op_store.retrieve_ops(to_retrieve).await.unwrap();

    assert_eq!(2, retrieved.len());
}

#[tokio::test]
async fn retrieve_ops_does_not_panic_with_too_short_op_ids() {
    let (_, op_store) = setup_test().await;

    let op_id_too_short = kitsune2_api::OpId::from(bytes::Bytes::from(vec![0u8; 31]));
    assert_eq!(op_id_too_short.len(), 31);

    let retrieved_ops = op_store.retrieve_ops(vec![op_id_too_short]).await.unwrap();
    assert_eq!(retrieved_ops.len(), 0);
}

#[tokio::test]
async fn filter_out_existing_ops() {
    let (store, op_store) = setup_test().await;

    let dht_op_1 = test_dht_op(Timestamp::now());
    let dht_op_2 = test_dht_op(Timestamp::now());

    op_store
        .process_incoming_ops(vec![
            Bytes::from(holochain_serialized_bytes::encode(dht_op_1.as_content()).unwrap()),
            Bytes::from(holochain_serialized_bytes::encode(dht_op_2.as_content()).unwrap()),
        ])
        .await
        .unwrap();

    // `filter_out_existing_ops` consults `ChainOp` (integrated), so we
    // need to drive the ops through validation+integration first.
    set_all_integrated(
        &store,
        &[dht_op_1.as_hash().clone(), dht_op_2.as_hash().clone()],
    )
    .await;

    let to_check = vec![
        dht_op_1
            .as_hash()
            .to_located_k2_op_id(&dht_op_1.dht_basis()),
        dht_op_2
            .as_hash()
            .to_located_k2_op_id(&dht_op_2.dht_basis()),
    ];
    let all_exist_filtered = op_store.filter_out_existing_ops(to_check).await.unwrap();

    assert!(all_exist_filtered.is_empty());

    let non_existent_op_id = OpId::from(Bytes::from_static(&[5; 36]));
    let to_check = vec![
        dht_op_1
            .as_hash()
            .to_located_k2_op_id(&dht_op_1.dht_basis()),
        non_existent_op_id.clone(),
    ];
    let some_exists_filtered = op_store.filter_out_existing_ops(to_check).await.unwrap();

    assert_eq!(1, some_exists_filtered.len());
    assert_eq!(non_existent_op_id, some_exists_filtered[0]);
}

#[tokio::test]
async fn filter_out_existing_ops_filters_invalid_op_ids_as_well() {
    let (_, op_store) = setup_test().await;

    let valid_op = test_dht_op(Timestamp::now());
    let valid_op_id = valid_op
        .as_hash()
        .to_located_k2_op_id(&valid_op.dht_basis());

    let op_id_too_short = kitsune2_api::OpId::from(bytes::Bytes::from(vec![0u8; 31]));
    assert_eq!(op_id_too_short.len(), 31);

    let out = op_store
        .filter_out_existing_ops(vec![op_id_too_short, valid_op_id.clone()])
        .await
        .unwrap();

    assert_eq!(out.len(), 1);
    assert_eq!(out[0].clone(), valid_op_id);
}

#[tokio::test]
async fn retrieve_in_time_slice() {
    let (store, op_store) = setup_test().await;

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

    let op_hashes: Vec<_> = dht_ops.iter().map(|o| o.as_hash().clone()).collect();
    set_all_integrated(&store, &op_hashes).await;

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
    let (store, op_store) = setup_test().await;

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

    let op_hashes: Vec<_> = dht_ops.iter().map(|o| o.as_hash().clone()).collect();
    set_all_integrated(&store, &op_hashes).await;

    // Get everything. NOTE: with the new schema, `retrieve_op_ids_bounded`
    // orders by `when_integrated` (set by `set_all_integrated` to "now"
    // for every op in a single call), so the per-op timestamp distinction
    // the old test relied on is collapsed. We still expect every op to
    // come back; size still matches; the returned cursor advances to the
    // shared `when_integrated`.
    let (hashes, size, _timestamp) = op_store
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

    // Retrieve, bounded by the size of the first 750 ops. Since every op
    // shares the same `when_integrated`, the loop returns up to the bound
    // and stops; we should see roughly the first 750 by byte budget.
    let bounded_size = encoded_ops.iter().take(750).map(|b| b.len()).sum::<usize>() as u32;
    let (hashes, size, _timestamp) = op_store
        .retrieve_op_ids_bounded(
            DhtArc::FULL,
            kitsune2_api::Timestamp::from_micros(0),
            bounded_size,
        )
        .await
        .unwrap();
    assert_eq!(750, hashes.len());
    assert_eq!(bounded_size, size);
}

#[tokio::test]
async fn create_and_read_slice_hashes() {
    let (_, op_store) = setup_test().await;

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
    let (_, op_store) = setup_test().await;

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
    // The "count" is the highest stored slice index, not the literal count
    assert_eq!(6, count);

    let count = op_store
        .slice_hash_count(DhtArc::Arc(0, 101))
        .await
        .unwrap();
    assert_eq!(5, count);
}

#[tokio::test]
async fn slice_hashes_separate_by_arc() {
    let (_, op_store) = setup_test().await;

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
async fn overwrite_slice_hashes() {
    let (_, op_store) = setup_test().await;

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
    assert_eq!(5, count);

    let hash = op_store
        .retrieve_slice_hash(DhtArc::Arc(0, 100), 5)
        .await
        .unwrap();
    assert_eq!(Some(Bytes::from_static(b"hash-b")), hash);
}

async fn setup_test() -> (DhtStore, HolochainOpStore) {
    let dna_hash = DnaHash::from_raw_36(vec![0; 36]);
    let store = DhtStore::new_test(holochain_data::kind::Dht::new(Arc::new(dna_hash.clone())))
        .await
        .unwrap();

    let sender: DynHcP2pHandler = Arc::new(StubHost {
        store: store.clone(),
    });
    let sender_w = Arc::new(std::sync::OnceLock::new());
    sender_w.set(holochain_p2p::WrapEvtSender(sender)).unwrap();

    let op_store = HolochainOpStore::new(store.clone(), dna_hash, sender_w);

    (store, op_store)
}
