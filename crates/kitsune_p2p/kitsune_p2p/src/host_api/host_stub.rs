use super::*;
use crate::KitsuneHostDefaultError;
use futures::FutureExt;
use kitsune_p2p_block::{Block, BlockTarget, BlockTargetId};
use kitsune_p2p_fetch::*;
use kitsune_p2p_timestamp::Timestamp;
use std::cell::RefCell;
use std::collections::HashSet;

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
            blocks: Arc::new(parking_lot::Mutex::new(HashSet::new())),
        })
    }

    /// Constructor
    pub fn with_check_op_data(check_op_data_impl: CheckOpDataImpl) -> std::sync::Arc<Self> {
        std::sync::Arc::new(Self {
            err: HostStubErr,
            check_op_data_impl: Some(check_op_data_impl),
            blocks: Arc::new(parking_lot::Mutex::new(HashSet::new())),
        })
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
        KitsuneHostDefaultError::get_agent_info_signed(&self.err, input)
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
        KitsuneHostDefaultError::op_hash(&self.err, op_data)
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
