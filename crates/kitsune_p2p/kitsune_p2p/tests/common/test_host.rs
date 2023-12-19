use std::sync::Arc;

use super::data::TestHostOp;
use futures::FutureExt;
use kitsune_p2p::{KitsuneHost, KitsuneP2pResult};
use kitsune_p2p_timestamp::Timestamp;
use kitsune_p2p_types::{
    agent_info::AgentInfoSigned,
    dht::{
        arq::ArqSet,
        hash::RegionHash,
        region::RegionData,
        region_set::{RegionCoordSetLtcs, RegionSetLtcs},
        spacetime::{Dimension, TelescopingTimes, Topology},
        ArqStrat,
    },
};

#[derive(Debug, Clone)]
pub struct TestHost {
    agent_store: Arc<parking_lot::RwLock<Vec<AgentInfoSigned>>>,
    op_store: Arc<parking_lot::RwLock<Vec<TestHostOp>>>,
}

impl TestHost {
    pub fn new(
        agent_store: Arc<parking_lot::RwLock<Vec<AgentInfoSigned>>>,
        op_store: Arc<parking_lot::RwLock<Vec<TestHostOp>>>,
    ) -> Self {
        Self {
            agent_store,
            op_store,
        }
    }
}

impl KitsuneHost for TestHost {
    fn block(&self, _input: kitsune_p2p_block::Block) -> kitsune_p2p::KitsuneHostResult<()> {
        todo!()
    }

    fn unblock(&self, _input: kitsune_p2p_block::Block) -> kitsune_p2p::KitsuneHostResult<()> {
        todo!()
    }

    fn is_blocked(
        &self,
        _input: kitsune_p2p_block::BlockTargetId,
        _timestamp: kitsune_p2p_types::dht::prelude::Timestamp,
    ) -> kitsune_p2p::KitsuneHostResult<bool> {
        // TODO implement me
        async move { Ok(false) }.boxed().into()
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
        _input: kitsune_p2p::event::GetAgentInfoSignedEvt,
    ) -> kitsune_p2p::KitsuneHostResult<bool> {
        todo!()
    }

    fn peer_extrapolated_coverage(
        &self,
        _space: Arc<kitsune_p2p_bin_data::KitsuneSpace>,
        _dht_arc_set: kitsune_p2p_types::dht_arc::DhtArcSet,
    ) -> kitsune_p2p::KitsuneHostResult<Vec<f64>> {
        // TODO implement me
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
                    let (x0, x1) = bounds.x;
                    let (t0, t1) = bounds.t;

                    Ok(self
                        .op_store
                        .read()
                        .iter()
                        .filter(|op| {
                            let loc = op.location();
                            let time = op.authored_at();
                            if x0 <= x1 {
                                if loc < x0 || loc > x1 {
                                    return false;
                                }
                            } else {
                                if loc > x0 && loc < x1 {
                                    return false;
                                }
                            }

                            time >= t0 && time <= t1
                        })
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
        _space: Arc<kitsune_p2p_bin_data::KitsuneSpace>,
        _region: kitsune_p2p_types::dht::prelude::RegionCoords,
    ) -> kitsune_p2p::KitsuneHostResult<Vec<kitsune_p2p_fetch::OpHashSized>> {
        todo!()
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
        let cutoff = std::time::Duration::from_secs(60 * 15);
        async move {
            Ok(Topology {
                space: Dimension::standard_space(),
                time: Dimension::time(std::time::Duration::from_secs(60 * 5)),
                time_origin: Timestamp::now(),
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
}
