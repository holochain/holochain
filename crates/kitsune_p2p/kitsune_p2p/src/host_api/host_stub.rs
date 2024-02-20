use super::*;
use crate::event::KitsuneP2pEvent;
use crate::test_util::data::mk_agent_info;
use crate::{KitsuneBinType, KitsuneHostDefaultError};
use futures::FutureExt;
use kitsune_p2p_block::{Block, BlockTarget, BlockTargetId};
use kitsune_p2p_fetch::*;
use kitsune_p2p_timestamp::Timestamp;
use kitsune_p2p_types::bin_types::KitsuneOpHash;
use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

/// Signature for check_op_data_impl
pub type CheckOpDataImpl = Box<
    dyn Fn(
            Arc<KitsuneSpace>,
            Vec<KOpHash>,
            Option<FetchContext>,
        ) -> KitsuneHostResult<'static, Vec<bool>>
        + 'static
        + Send
        + Sync,
>;

#[derive(Debug)]
struct HostStubErr;

impl KitsuneHostDefaultError for HostStubErr {
    const NAME: &'static str = "HostStub";
}

impl FetchPoolConfig for HostStubErr {
    fn merge_fetch_contexts(&self, _a: u32, _b: u32) -> u32 {
        unimplemented!()
    }
}

/// Dummy host impl for plumbing
pub struct HostStub {
    err: HostStubErr,
    check_op_data_impl: Option<CheckOpDataImpl>,
    fail_next_request: Arc<AtomicBool>,
    fail_count: Arc<AtomicUsize>,
    blocks: Arc<parking_lot::Mutex<HashSet<Block>>>,
}

/// Manual implementation of debug to skip over underivable Debug field.
impl std::fmt::Debug for HostStub {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HostStub").field("err", &self.err).finish()
    }
}

impl HostStub {
    /// Constructor
    pub fn new() -> std::sync::Arc<Self> {
        std::sync::Arc::new(Self {
            err: HostStubErr,
            check_op_data_impl: None,
            fail_next_request: Arc::new(AtomicBool::new(false)),
            fail_count: Arc::new(AtomicUsize::new(0)),
            blocks: Arc::new(parking_lot::Mutex::new(HashSet::new())),
        })
    }

    /// Constructor
    pub fn with_check_op_data(check_op_data_impl: CheckOpDataImpl) -> std::sync::Arc<Self> {
        std::sync::Arc::new(Self {
            err: HostStubErr,
            check_op_data_impl: Some(check_op_data_impl),
            fail_next_request: Arc::new(AtomicBool::new(false)),
            fail_count: Arc::new(AtomicUsize::new(0)),
            blocks: Arc::new(parking_lot::Mutex::new(HashSet::new())),
        })
    }

    /// Request that the next request will fail and respond with an error
    pub fn fail_next_request(&self) {
        self.fail_next_request.store(true, Ordering::SeqCst);
    }

    /// Get the count of requests that have failed due to `fail_next_request`.
    pub fn get_fail_count(&self) -> usize {
        self.fail_count.load(Ordering::SeqCst)
    }

    /// Wrap it up with a legacy sender
    pub fn legacy(
        self: Arc<Self>,
        sender: futures::channel::mpsc::Sender<KitsuneP2pEvent>,
    ) -> HostApiLegacy {
        HostApiLegacy::new(self, sender)
    }
}

impl KitsuneHost for HostStub {
    fn block(&self, input: Block) -> KitsuneHostResult<()> {
        let mut blocks = self.blocks.lock();
        blocks.insert(input);

        async move { Ok(()) }.boxed().into()
    }

    fn unblock(&self, input: Block) -> KitsuneHostResult<()> {
        let mut blocks = self.blocks.lock();
        blocks.remove(&input);

        async move { Ok(()) }.boxed().into()
    }

    fn is_blocked(
        &self,
        input: kitsune_p2p_block::BlockTargetId,
        timestamp: Timestamp,
    ) -> crate::KitsuneHostResult<bool> {
        let blocks = self.blocks.lock();

        let blocked = match &input {
            BlockTargetId::Node(check_node_id) => {
                let maybe_matched_block = blocks.iter().find(|b| match b.target() {
                    BlockTarget::Node(node_id, _) => node_id == check_node_id,
                    _ => false,
                });

                if let Some(block) = maybe_matched_block {
                    timestamp.0 > block.start().0 && timestamp.0 < block.end().0
                } else {
                    false
                }
            }
            _ => false,
        };

        async move { Ok(blocked) }.boxed().into()
    }

    fn get_agent_info_signed(
        &self,
        input: GetAgentInfoSignedEvt,
    ) -> KitsuneHostResult<Option<crate::types::agent_store::AgentInfoSigned>> {
        if let Ok(true) =
            self.fail_next_request
                .compare_exchange(true, false, Ordering::SeqCst, Ordering::SeqCst)
        {
            self.fail_count.fetch_add(1, Ordering::SeqCst);
            return KitsuneHostDefaultError::get_agent_info_signed(&self.err, input);
        }

        async move {
            let signed = mk_agent_info(*input.agent.0.to_vec().first().unwrap()).await;
            Ok(Some(signed))
        }
        .boxed()
        .into()
    }

    fn remove_agent_info_signed(&self, input: GetAgentInfoSignedEvt) -> KitsuneHostResult<bool> {
        KitsuneHostDefaultError::remove_agent_info_signed(&self.err, input)
    }

    fn peer_extrapolated_coverage(
        &self,
        space: Arc<KitsuneSpace>,
        dht_arc_set: DhtArcSet,
    ) -> KitsuneHostResult<Vec<f64>> {
        KitsuneHostDefaultError::peer_extrapolated_coverage(&self.err, space, dht_arc_set)
    }

    fn query_region_set(
        &self,
        space: Arc<KitsuneSpace>,
        dht_arc_set: Arc<DhtArcSet>,
    ) -> KitsuneHostResult<RegionSetLtcs> {
        KitsuneHostDefaultError::query_region_set(&self.err, space, dht_arc_set)
    }

    fn query_size_limited_regions(
        &self,
        space: Arc<KitsuneSpace>,
        size_limit: u32,
        regions: Vec<Region>,
    ) -> crate::KitsuneHostResult<Vec<Region>> {
        KitsuneHostDefaultError::query_size_limited_regions(&self.err, space, size_limit, regions)
    }

    fn query_op_hashes_by_region(
        &self,
        space: Arc<KitsuneSpace>,
        region: RegionCoords,
    ) -> KitsuneHostResult<Vec<OpHashSized>> {
        KitsuneHostDefaultError::query_op_hashes_by_region(&self.err, space, region)
    }

    fn record_metrics(
        &self,
        space: Arc<KitsuneSpace>,
        records: Vec<MetricRecord>,
    ) -> KitsuneHostResult<()> {
        KitsuneHostDefaultError::record_metrics(&self.err, space, records)
    }

    fn get_topology(&self, space: Arc<KitsuneSpace>) -> KitsuneHostResult<Topology> {
        KitsuneHostDefaultError::get_topology(&self.err, space)
    }

    fn op_hash(&self, op_data: KOpData) -> KitsuneHostResult<KOpHash> {
        if let Ok(true) =
            self.fail_next_request
                .compare_exchange(true, false, Ordering::SeqCst, Ordering::SeqCst)
        {
            self.fail_count.fetch_add(1, Ordering::SeqCst);
            return KitsuneHostDefaultError::op_hash(&self.err, op_data);
        }

        async move {
            // Probably not important but we could compute a real hash here if a test needs it
            let hash_byte = op_data.0.first().cloned().unwrap_or(0);
            Ok(Arc::new(KitsuneOpHash::new(vec![hash_byte; 36])))
        }
        .boxed()
        .into()
    }

    fn check_op_data(
        &self,
        space: Arc<KitsuneSpace>,
        op_hash_list: Vec<KOpHash>,
        context: Option<kitsune_p2p_fetch::FetchContext>,
    ) -> KitsuneHostResult<Vec<bool>> {
        if let Some(i) = &self.check_op_data_impl {
            i(space, op_hash_list, context)
        } else {
            KitsuneHost::check_op_data(&self.err, space, op_hash_list, context)
        }
    }
}

impl FetchPoolConfig for HostStub {
    fn merge_fetch_contexts(&self, _a: u32, _b: u32) -> u32 {
        unimplemented!()
    }
}
