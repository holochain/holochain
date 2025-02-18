use bytes::Bytes;
use fixt::fixt;
use futures::FutureExt;
use ghost_actor::actor_builder::GhostActorBuilder;
use ghost_actor::{GhostControlHandler, GhostHandler};
use holo_hash::{AgentPubKey, AnyDhtHash, DhtOpHash, DnaHash, HasHash};
use holochain_p2p::dht::PeerView;
use holochain_p2p::dht_arc::DhtArcSet;
use holochain_p2p::event::{
    CountersigningSessionNegotiationMessage, FetchOpDataQuery, GetActivityOptions, GetLinksOptions,
    GetMetaOptions, GetOptions, HolochainP2pEvent, HolochainP2pEventHandler,
    HolochainP2pEventHandlerResult,
};
use holochain_p2p::HolochainOpStore;
use holochain_serialized_bytes::SerializedBytes;
use holochain_sqlite::db::{DbKindDht, DbWrite};
use holochain_state::prelude::{
    ChainFilter, ExternIO, RecordEntry, Signature, StateMutationResult,
};
use holochain_timestamp::Timestamp;
use holochain_types::activity::AgentActivityResponse;
use holochain_types::chain::MustGetAgentActivityResponse;
use holochain_types::dht_op::{ChainOp, DhtOpHashed, WireOps};
use holochain_types::link::{CountLinksResponse, WireLinkKey, WireLinkOps, WireLinkQuery};
use holochain_types::metadata::MetadataSet;
use holochain_types::prelude::{DhtOp, ValidationReceiptBundle};
use holochain_zome_types::fixt::{CreateFixturator, EntryFixturator, SignatureFixturator};
use holochain_zome_types::prelude::ChainQueryFilter;
use holochain_zome_types::Action;
use kitsune2_api::{DhtArc, OpId, OpStore};
use kitsune_p2p::event::{TimeWindow, TimeWindowInclusive};
use kitsune_p2p::{KitsuneAgent, KitsuneSpace};
use kitsune_p2p_types::agent_info::AgentInfoSigned;
use kitsune_p2p_types::bootstrap::AgentInfoPut;
use std::collections::HashSet;
use std::sync::Arc;

struct StubHost {
    db: DbWrite<DbKindDht>,
}

impl GhostControlHandler for StubHost {}
impl GhostHandler<HolochainP2pEvent> for StubHost {}

impl HolochainP2pEventHandler for StubHost {
    fn handle_put_agent_info_signed(
        &mut self,
        _dna_hash: DnaHash,
        _peer_data: Vec<AgentInfoSigned>,
    ) -> HolochainP2pEventHandlerResult<Vec<AgentInfoPut>> {
        unimplemented!()
    }

    fn handle_query_agent_info_signed(
        &mut self,
        _dna_hash: DnaHash,
        _agents: Option<HashSet<Arc<KitsuneAgent>>>,
        _kitsune_space: Arc<KitsuneSpace>,
    ) -> HolochainP2pEventHandlerResult<Vec<AgentInfoSigned>> {
        unimplemented!()
    }

    fn handle_query_gossip_agents(
        &mut self,
        _dna_hash: DnaHash,
        _agents: Option<Vec<AgentPubKey>>,
        _kitsune_space: Arc<KitsuneSpace>,
        _since_ms: u64,
        _until_ms: u64,
        _arc_set: Arc<DhtArcSet>,
    ) -> HolochainP2pEventHandlerResult<Vec<AgentInfoSigned>> {
        unimplemented!()
    }

    fn handle_query_agent_info_signed_near_basis(
        &mut self,
        _dna_hash: DnaHash,
        _kitsune_space: Arc<KitsuneSpace>,
        _basis_loc: u32,
        _limit: u32,
    ) -> HolochainP2pEventHandlerResult<Vec<AgentInfoSigned>> {
        unimplemented!()
    }

    fn handle_query_peer_density(
        &mut self,
        _dna_hash: DnaHash,
        _kitsune_space: Arc<KitsuneSpace>,
        _dht_arc: holochain_p2p::dht_arc::DhtArc,
    ) -> HolochainP2pEventHandlerResult<PeerView> {
        unimplemented!()
    }

    fn handle_call_remote(
        &mut self,
        _dna_hash: DnaHash,
        _to_agent: AgentPubKey,
        _zome_call_params_serialized: ExternIO,
        _signature: Signature,
    ) -> HolochainP2pEventHandlerResult<SerializedBytes> {
        unimplemented!()
    }

    fn handle_publish(
        &mut self,
        _dna_hash: DnaHash,
        _request_validation_receipt: bool,
        _countersigning_session: bool,
        ops: Vec<DhtOp>,
    ) -> HolochainP2pEventHandlerResult<()> {
        let db = self.db.clone();

        Ok(async move {
            db.write_async(move |txn| -> StateMutationResult<()> {
                for op in ops {
                    let size = holochain_serialized_bytes::encode(&op).unwrap().len();
                    holochain_state::prelude::insert_op_dht(
                        txn,
                        &DhtOpHashed::from_content_sync(op),
                        size as u32,
                        None,
                    )?;
                }

                Ok(())
            })
            .await
            .unwrap();

            Ok(())
        }
        .boxed()
        .into())
    }

    fn handle_get(
        &mut self,
        _dna_hash: DnaHash,
        _to_agent: AgentPubKey,
        _dht_hash: AnyDhtHash,
        _options: GetOptions,
    ) -> HolochainP2pEventHandlerResult<WireOps> {
        unimplemented!()
    }

    fn handle_get_meta(
        &mut self,
        _dna_hash: DnaHash,
        _to_agent: AgentPubKey,
        _dht_hash: AnyDhtHash,
        _options: GetMetaOptions,
    ) -> HolochainP2pEventHandlerResult<MetadataSet> {
        unimplemented!()
    }

    fn handle_get_links(
        &mut self,
        _dna_hash: DnaHash,
        _to_agent: AgentPubKey,
        _link_key: WireLinkKey,
        _options: GetLinksOptions,
    ) -> HolochainP2pEventHandlerResult<WireLinkOps> {
        unimplemented!()
    }

    fn handle_count_links(
        &mut self,
        _dna_hash: DnaHash,
        _to_agent: AgentPubKey,
        _query: WireLinkQuery,
    ) -> HolochainP2pEventHandlerResult<CountLinksResponse> {
        unimplemented!()
    }

    fn handle_get_agent_activity(
        &mut self,
        _dna_hash: DnaHash,
        _to_agent: AgentPubKey,
        _agent: AgentPubKey,
        _query: ChainQueryFilter,
        _options: GetActivityOptions,
    ) -> HolochainP2pEventHandlerResult<AgentActivityResponse> {
        unimplemented!()
    }

    fn handle_must_get_agent_activity(
        &mut self,
        _dna_hash: DnaHash,
        _to_agent: AgentPubKey,
        _author: AgentPubKey,
        _filter: ChainFilter,
    ) -> HolochainP2pEventHandlerResult<MustGetAgentActivityResponse> {
        unimplemented!()
    }

    fn handle_validation_receipts_received(
        &mut self,
        _dna_hash: DnaHash,
        _to_agent: AgentPubKey,
        _receipts: ValidationReceiptBundle,
    ) -> HolochainP2pEventHandlerResult<()> {
        unimplemented!()
    }

    fn handle_query_op_hashes(
        &mut self,
        _dna_hash: DnaHash,
        _arc_set: DhtArcSet,
        _window: TimeWindow,
        _max_ops: usize,
        _include_limbo: bool,
    ) -> HolochainP2pEventHandlerResult<Option<(Vec<DhtOpHash>, TimeWindowInclusive)>> {
        unimplemented!()
    }

    fn handle_fetch_op_data(
        &mut self,
        _dna_hash: DnaHash,
        _query: FetchOpDataQuery,
    ) -> HolochainP2pEventHandlerResult<Vec<(DhtOpHash, DhtOp)>> {
        unimplemented!()
    }

    fn handle_sign_network_data(
        &mut self,
        _dna_hash: DnaHash,
        _to_agent: AgentPubKey,
        _data: Vec<u8>,
    ) -> HolochainP2pEventHandlerResult<Signature> {
        unimplemented!()
    }

    fn handle_countersigning_session_negotiation(
        &mut self,
        _dna_hash: DnaHash,
        _to_agent: AgentPubKey,
        _message: CountersigningSessionNegotiationMessage,
    ) -> HolochainP2pEventHandlerResult<()> {
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

#[tokio::test]
async fn process_incoming_ops_and_retrieve() {
    let (db, op_store) = setup_test().await;

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
    let (db, op_store) = setup_test().await;

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
    let (db, op_store) = setup_test().await;

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

async fn setup_test() -> (DbWrite<DbKindDht>, HolochainOpStore) {
    let dna_hash = DnaHash::from_raw_36(vec![0; 36]);
    let db = DbWrite::test_in_mem(DbKindDht(Arc::new(dna_hash.clone()))).unwrap();

    let builder = GhostActorBuilder::new();
    let channel_factory = builder.channel_factory().clone();
    let sender = channel_factory
        .create_channel::<HolochainP2pEvent>()
        .await
        .unwrap();
    tokio::spawn(builder.spawn(StubHost { db: db.clone() }));

    let op_store = HolochainOpStore::new(db.clone(), dna_hash, sender);

    (db, op_store)
}

async fn set_all_integrated(db: DbWrite<DbKindDht>) {
    db.write_async(move |txn| -> StateMutationResult<()> {
        txn.execute("UPDATE DhtOp SET when_integrated = authored_timestamp", [])?;

        Ok(())
    })
    .await
    .unwrap();
}
