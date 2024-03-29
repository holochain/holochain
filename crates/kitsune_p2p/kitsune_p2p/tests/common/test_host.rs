use super::data::TestHostOp;
use futures::FutureExt;
use kitsune_p2p::{KitsuneHost, KitsuneP2pResult};
use kitsune_p2p_block::BlockTargetId;
use kitsune_p2p_timestamp::Timestamp;
use kitsune_p2p_types::{
    agent_info::AgentInfoSigned,
    config::RECENT_THRESHOLD_DEFAULT,
    dependencies::lair_keystore_api::LairClient,
    dht::{
        arq::ArqSet,
        hash::RegionHash,
        region::RegionData,
        region_set::{RegionCoordSetLtcs, RegionSetLtcs},
        spacetime::*,
        ArqStrat,
    },
};
use std::sync::Arc;

#[derive(Clone)]
pub struct TestHost {
    tag: String,
    keystore: LairClient,
    agent_store: Arc<parking_lot::RwLock<Vec<AgentInfoSigned>>>,
    op_store: Arc<parking_lot::RwLock<Vec<TestHostOp>>>,
    blocks: Arc<parking_lot::RwLock<Vec<kitsune_p2p_block::Block>>>,
}

impl std::fmt::Debug for TestHost {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TestHost")
            .field("agent_store", &self.agent_store.read())
            .field("op_store", &self.op_store.read())
            .finish()
    }
}

impl TestHost {
    pub async fn new(
        keystore: LairClient,
        agent_store: Arc<parking_lot::RwLock<Vec<AgentInfoSigned>>>,
        op_store: Arc<parking_lot::RwLock<Vec<TestHostOp>>>,
    ) -> Self {
        let tag = nanoid::nanoid!();
        keystore
            .new_seed(tag.clone().into(), None, false)
            .await
            .expect("Could not register lair seed");

        Self {
            tag,
            keystore,
            agent_store,
            op_store,
            blocks: Arc::new(parking_lot::RwLock::new(vec![])),
        }
    }
}

impl KitsuneHost for TestHost {
    fn block(&self, input: kitsune_p2p_block::Block) -> kitsune_p2p::KitsuneHostResult<()> {
        self.blocks.write().push(input);

        async move { Ok(()) }.boxed().into()
    }

    fn unblock(&self, input: kitsune_p2p_block::Block) -> kitsune_p2p::KitsuneHostResult<()> {
        self.blocks.write().retain(|b| b != &input);

        async move { Ok(()) }.boxed().into()
    }

    fn is_blocked(
        &self,
        input: kitsune_p2p_block::BlockTargetId,
        timestamp: kitsune_p2p_types::dht::prelude::Timestamp,
    ) -> kitsune_p2p::KitsuneHostResult<bool> {
        let blocked = self
            .blocks
            .read()
            .iter()
            .find(|b| {
                let target_id: BlockTargetId = b.target().clone().into();

                target_id == input && b.start() <= timestamp && b.end() >= timestamp
            })
            .is_some();

        async move { Ok(blocked) }.boxed().into()
    }

    fn get_agent_info_signed(
        &self,
        input: kitsune_p2p::event::GetAgentInfoSignedEvt,
    ) -> kitsune_p2p::KitsuneHostResult<Option<AgentInfoSigned>> {
        let res = self
            .agent_store
            .read()
            .iter()
            .find(|p| p.agent == input.agent)
            .cloned();

        async move { Ok(res) }.boxed().into()
    }

    fn remove_agent_info_signed(
        &self,
        input: kitsune_p2p::event::GetAgentInfoSignedEvt,
    ) -> kitsune_p2p::KitsuneHostResult<bool> {
        self.agent_store.write().retain(|p| p.agent != input.agent);

        // TODO This boolean return doesn't seem to be documented, what does it mean?
        async move { Ok(true) }.boxed().into()
    }

    fn peer_extrapolated_coverage(
        &self,
        _space: Arc<kitsune_p2p_bin_data::KitsuneSpace>,
        _dht_arc_set: kitsune_p2p_types::dht_arc::DhtArcSet,
    ) -> kitsune_p2p::KitsuneHostResult<Vec<f64>> {
        // This is only used for metrics, so just return a dummy value
        async move { Ok(vec![]) }.boxed().into()
    }

    fn query_region_set(
        &self,
        space: Arc<kitsune_p2p_bin_data::KitsuneSpace>,
        dht_arc_set: Arc<kitsune_p2p_types::dht_arc::DhtArcSet>,
    ) -> kitsune_p2p::KitsuneHostResult<kitsune_p2p_types::dht::prelude::RegionSetLtcs> {
        async move {
            let topology = self.get_topology(space.clone()).await?;

            let arq_set =
                ArqSet::from_dht_arc_set_exact(&topology, &ArqStrat::default(), &dht_arc_set)
                    .ok_or_else(|| -> KitsuneP2pResult<()> {
                        Err("Could not create arc set".into())
                    })
                    .unwrap();

            let times = TelescopingTimes::historical(&topology);
            let coords = RegionCoordSetLtcs::new(times, arq_set);

            let region_set: RegionSetLtcs<RegionData> = coords
                .into_region_set(|(_, coords)| -> KitsuneP2pResult<RegionData> {
                    let bounds = coords.to_bounds(&topology);

                    Ok(self
                        .op_store
                        .read()
                        .iter()
                        .filter(|op| op.is_in_bounds(&bounds))
                        .fold(
                            RegionData {
                                hash: RegionHash::from_vec(vec![0; 32]).unwrap(),
                                size: 0,
                                count: 0,
                            },
                            |acc, op| {
                                let mut current_hash = acc.hash.to_vec();
                                let op_hash = op.hash();
                                for i in 0..32 {
                                    current_hash[i] ^= op_hash[i];
                                }
                                RegionData {
                                    hash: RegionHash::from_vec(current_hash.to_vec()).unwrap(),
                                    size: acc.size + op.size(),
                                    count: acc.count + 1,
                                }
                            },
                        ))
                })
                .unwrap();

            Ok(region_set)
        }
        .boxed()
        .into()
    }

    // TODO This is never called, can it be removed or is it for future use?
    fn query_size_limited_regions(
        &self,
        _space: Arc<kitsune_p2p_bin_data::KitsuneSpace>,
        _size_limit: u32,
        _regions: Vec<kitsune_p2p_types::dht::prelude::Region>,
    ) -> kitsune_p2p::KitsuneHostResult<Vec<kitsune_p2p_types::dht::prelude::Region>> {
        todo!()
    }

    fn query_op_hashes_by_region(
        &self,
        space: Arc<kitsune_p2p_bin_data::KitsuneSpace>,
        region: kitsune_p2p_types::dht::prelude::RegionCoords,
    ) -> kitsune_p2p::KitsuneHostResult<Vec<kitsune_p2p_fetch::OpHashSized>> {
        async move {
            let topology = self.get_topology(space).await?;
            let bounds = region.to_bounds(&topology);

            Ok(self
                .op_store
                .read()
                .iter()
                .filter_map(|op| {
                    if op.is_in_bounds(&bounds) {
                        Some(op.clone().into())
                    } else {
                        None
                    }
                })
                .collect::<Vec<_>>())
        }
        .boxed()
        .into()
    }

    fn record_metrics(
        &self,
        _space: Arc<kitsune_p2p_bin_data::KitsuneSpace>,
        _records: Vec<kitsune_p2p_types::metrics::MetricRecord>,
    ) -> kitsune_p2p::KitsuneHostResult<()> {
        todo!()
    }

    fn get_topology(
        &self,
        _space: Arc<kitsune_p2p_bin_data::KitsuneSpace>,
    ) -> kitsune_p2p::KitsuneHostResult<kitsune_p2p_types::dht::prelude::Topology> {
        let cutoff = RECENT_THRESHOLD_DEFAULT;
        async move {
            Ok(Topology {
                space: SpaceDimension::standard(),
                time: TimeDimension::new(std::time::Duration::from_secs(60 * 5)),
                time_origin: Timestamp::ZERO,
                time_cutoff: cutoff,
            })
        }
        .boxed()
        .into()
    }

    fn op_hash(
        &self,
        op_data: kitsune_p2p_types::KOpData,
    ) -> kitsune_p2p::KitsuneHostResult<kitsune_p2p_types::KOpHash> {
        let op: TestHostOp = op_data.into();
        async move { Ok(Arc::new(op.kitsune_hash())) }
            .boxed()
            .into()
    }

    fn check_op_data(
        &self,
        space: Arc<kitsune_p2p_bin_data::KitsuneSpace>,
        op_hash_list: Vec<kitsune_p2p_types::KOpHash>,
        _context: Option<kitsune_p2p_fetch::FetchContext>,
    ) -> kitsune_p2p::KitsuneHostResult<Vec<bool>> {
        let res = op_hash_list
            .iter()
            .map(|op_hash| {
                self.op_store
                    .read()
                    .iter()
                    .any(|op| op.space() == space && &Arc::new(op.kitsune_hash()) == op_hash)
            })
            .collect();

        async move { Ok(res) }.boxed().into()
    }

    fn lair_tag(&self) -> Option<Arc<str>> {
        Some(self.tag.clone().into())
    }

    fn lair_client(
        &self,
    ) -> Option<kitsune_p2p_types::dependencies::lair_keystore_api::LairClient> {
        Some(self.keystore.clone())
    }
}
