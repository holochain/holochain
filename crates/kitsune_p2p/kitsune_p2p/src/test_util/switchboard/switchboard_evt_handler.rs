//! Gossip event handler which uses `SwitchboardState` as its underlying persisted store.

#![allow(clippy::unit_arg)]

use std::sync::Arc;

use crate::types::event::{KitsuneP2pEvent, KitsuneP2pEventHandler, KitsuneP2pEventHandlerResult};
use crate::{event::*, KitsuneHost};
use futures::FutureExt;
use kitsune_p2p_timestamp::Timestamp;
use kitsune_p2p_types::bin_types::*;
use kitsune_p2p_types::combinators::second;
use kitsune_p2p_types::dht::hash::RegionHash;
use kitsune_p2p_types::dht::prelude::{
    array_xor, ArqBoundsSet, RegionBounds, RegionCoordSetLtcs, RegionData,
};
use kitsune_p2p_types::dht::spacetime::{TelescopingTimes, TimeQuantum};
use kitsune_p2p_types::dht_arc::{DhtArc, DhtLocation};
use kitsune_p2p_types::*;

use super::switchboard_state::{NodeEp, NodeOpEntry, OpEntry, Switchboard};

type KSpace = Arc<KitsuneSpace>;
type KAgent = Arc<KitsuneAgent>;
type KOpHash = Arc<KitsuneOpHash>;

/// Stateful handler for KitsuneP2pEvents.
///
/// This is a very basic in-memory implementation of an event handler similar
/// to what a Kitsune implementor like Holochain would implement.
/// It's used to implement nodes in the Switchboard.
#[derive(Clone)]
pub struct SwitchboardEventHandler {
    node: NodeEp,
    sb: Switchboard,
}

impl SwitchboardEventHandler {
    /// Constructor
    pub fn new(node: NodeEp, sb: Switchboard) -> Self {
        Self { node, sb }
    }
}

impl ghost_actor::GhostHandler<KitsuneP2pEvent> for SwitchboardEventHandler {}
impl ghost_actor::GhostControlHandler for SwitchboardEventHandler {}

impl KitsuneHost for SwitchboardEventHandler {
    fn get_agent_info_signed(
        &self,
        GetAgentInfoSignedEvt { agent, space: _ }: GetAgentInfoSignedEvt,
    ) -> crate::KitsuneHostResult<Option<crate::types::agent_store::AgentInfoSigned>> {
        box_fut(Ok(self.sb.share(|state| {
            let node = state.nodes.get_mut(&self.node).unwrap();
            let loc = agent.get_loc().as_loc8();
            node.local_agents
                .get(&loc)
                .map(|e| e.info.to_owned())
                .or_else(|| node.remote_agents.get(&loc).cloned())
        })))
    }

    fn peer_extrapolated_coverage(
        &self,
        _space: Arc<KitsuneSpace>,
        _dht_arc_set: dht_arc::DhtArcSet,
    ) -> crate::KitsuneHostResult<Vec<f64>> {
        unimplemented!()
    }

    fn record_metrics(
        &self,
        _space: Arc<KitsuneSpace>,
        _records: Vec<MetricRecord>,
    ) -> crate::KitsuneHostResult<()> {
        box_fut(Ok(()))
    }

    fn query_size_limited_regions(
        &self,
        _space: Arc<KitsuneSpace>,
        _size_limit: u32,
        regions: Vec<dht::region::Region>,
    ) -> crate::KitsuneHostResult<Vec<dht::region::Region>> {
        // This false implementation will work fine as long as we're not trying
        // to test situations with regions with a large byte count getting broken up
        box_fut(Ok(regions))
    }

    fn query_region_set(
        &self,
        space: Arc<KitsuneSpace>,
        dht_arc_set: Arc<dht_arc::DhtArcSet>,
    ) -> crate::KitsuneHostResult<dht::region_set::RegionSetLtcs> {
        async move {
            let topo = self.get_topology(space).await?;
            let arq_set = ArqBoundsSet::from_dht_arc_set(&topo, &self.sb.strat, &dht_arc_set)
                .expect("an arq could not be quantized");

            // NOTE: If this were implemented correctly, it would take the recent_threshold
            //       (default 1 hour) into account, so that historical gossip doesn't overlap
            //       with recent gossip. But since the Switchboard only runs one gossip type
            //       at a time, it seems safer to just run historical gossip for all of time.
            let current = Timestamp::now();
            let times =
                TelescopingTimes::new(TimeQuantum::from_timestamp(&self.sb.topology, current));
            let coord_set = RegionCoordSetLtcs::new(times, arq_set);
            coord_set.into_region_set(|(_, coords)| {
                let bounds = coords.to_bounds(&self.sb.topology);
                let RegionBounds {
                    x: (x0, x1),
                    t: (t0, t1),
                } = bounds;
                let ops: Vec<_> = self.sb.share(|sb| {
                    let all_ops = &sb.ops;
                    let node_ops = &sb.nodes;

                    // let held: Vec<Loc8> = sb
                    // .nodes
                    // .get(&self.node)
                    // .unwrap()
                    // .ops
                    // .iter()
                    // .filter(|(_, o)| o.is_integrated)
                    // .map(|(loc8, _)| loc8)
                    // .collect();

                    all_ops
                        .iter()
                        .filter(move |(loc8, op)| {
                            let loc = DhtLocation::from(**loc8);
                            let arc = DhtArc::from_bounds(x0, x1);
                            let owned = node_ops
                                .get(&self.node)
                                .unwrap()
                                .ops
                                .get(loc8)
                                .map(|o| o.is_integrated)
                                .unwrap_or_default();
                            owned && arc.contains(&loc) && t0 <= op.timestamp && op.timestamp < t1
                        })
                        .map(second)
                        .cloned()
                        .collect()
                });
                let hash = ops.iter().fold([0; 32], |mut h, o| {
                    array_xor(&mut h, o.hash.get_bytes().try_into().unwrap());
                    h
                });

                Ok(RegionData {
                    hash: RegionHash::from(hash),
                    count: ops.len() as u32,
                    size: ops.len() as u32,
                })
            })
        }
        .boxed()
        .into()
    }

    fn get_topology(
        &self,
        _space: Arc<KitsuneSpace>,
    ) -> crate::KitsuneHostResult<dht::spacetime::Topology> {
        box_fut(Ok(self.sb.topology.clone()))
    }
}

#[allow(warnings)]
impl KitsuneP2pEventHandler for SwitchboardEventHandler {
    fn handle_put_agent_info_signed(
        &mut self,
        PutAgentInfoSignedEvt { space, peer_data }: PutAgentInfoSignedEvt,
    ) -> KitsuneP2pEventHandlerResult<()> {
        self.sb.share(|state| {
            state
                .nodes
                .get_mut(&self.node)
                .unwrap()
                .remote_agents
                .extend(
                    peer_data
                        .into_iter()
                        .map(|info| (info.agent.get_loc().as_loc8(), info)),
                );
        });
        ok_fut(Ok(()))
    }

    fn handle_query_agents(
        &mut self,
        QueryAgentsEvt {
            space,
            agents,
            window,
            arc_set,
            near_basis,
            limit,
        }: QueryAgentsEvt,
    ) -> KitsuneP2pEventHandlerResult<Vec<crate::types::agent_store::AgentInfoSigned>> {
        let result = self.sb.share(|state| {
            let node = &state.nodes.get(&self.node).expect("Node not added");
            let all_agents = node.all_agent_infos().into_iter();
            if let Some(agents) = agents {
                all_agents
                    .filter(|info| agents.contains(&info.agent))
                    .collect()
            } else {
                all_agents.collect()
            }
        });
        ok_fut(Ok(result))
    }

    fn handle_query_peer_density(
        &mut self,
        space: Arc<KitsuneSpace>,
        dht_arc: kitsune_p2p_types::dht_arc::DhtArc,
    ) -> KitsuneP2pEventHandlerResult<kitsune_p2p_types::dht::PeerView> {
        todo!()
    }

    fn handle_call(
        &mut self,
        space: Arc<KitsuneSpace>,
        to_agent: Arc<KitsuneAgent>,
        payload: Vec<u8>,
    ) -> KitsuneP2pEventHandlerResult<Vec<u8>> {
        todo!()
    }

    fn handle_notify(
        &mut self,
        space: Arc<KitsuneSpace>,
        to_agent: Arc<KitsuneAgent>,
        payload: Vec<u8>,
    ) -> KitsuneP2pEventHandlerResult<()> {
        todo!()
    }

    fn handle_gossip(
        &mut self,
        _space: Arc<KitsuneSpace>,
        ops: Vec<KOp>,
    ) -> KitsuneP2pEventHandlerResult<()> {
        ok_fut(Ok(self.sb.share(|sb| {
            let node = sb.nodes.get_mut(&self.node).unwrap();
            for op in ops {
                // As a hack, we just set the first bytes of the op data to the
                // loc8 location. This mimics the real world usage, where the
                // location would be able to be extracted from the real op data,
                // but is just a hack here since we're not even bothering to
                // deserialize.
                //
                // NB: this may be problematic on the receiving end because we
                // actually care whether this Loc8 is interpreted as u8 or i8,
                // and we lose that information here.
                let loc = op.0[0] as u8 as i32;
                if loc == 192 {
                    dbg!((&self.node, loc));
                }

                // TODO: allow setting integration status
                node.ops.insert(
                    loc.into(),
                    NodeOpEntry {
                        is_integrated: true,
                    },
                );
            }
        })))
    }

    fn handle_query_op_hashes(
        &mut self,
        QueryOpHashesEvt {
            space: _,
            arc_set,
            window,
            max_ops,
            include_limbo,
        }: QueryOpHashesEvt,
    ) -> KitsuneP2pEventHandlerResult<Option<(Vec<Arc<KitsuneOpHash>>, TimeWindowInclusive)>> {
        ok_fut(Ok(self.sb.share(|sb| {
            let (ops, timestamps): (Vec<_>, Vec<_>) = sb
                .get_ops_loc8(&self.node)
                .iter()
                .filter_map(|op_loc8| {
                    let op = sb.ops.get(op_loc8).unwrap();
                    (
                        // Does the op fall within the time window?
                        window.contains(&op.timestamp)
                        // Does the op fall within one of the specified arcsets
                        // with the correct integration/limbo criteria?
                        && arc_set.contains((*op_loc8).into())
                    )
                    .then(|| op)
                })
                .map(|op| (op.hash.clone(), op.timestamp))
                .take(max_ops)
                .unzip();

            if ops.is_empty() {
                None
            } else {
                let window = timestamps.into_iter().fold(
                    window.start..=window.end,
                    |mut window, timestamp| {
                        if timestamp < *window.start() {
                            window = timestamp..=*window.end();
                        }
                        if timestamp > *window.end() {
                            window = *window.start()..=timestamp;
                        }
                        window
                    },
                );
                Some((ops, window))
            }
        })))
    }

    fn handle_fetch_op_data(
        &mut self,
        FetchOpDataEvt { space, query }: FetchOpDataEvt,
    ) -> KitsuneP2pEventHandlerResult<Vec<(Arc<KitsuneOpHash>, KOp)>> {
        ok_fut(Ok(self.sb.share(|sb| match query {
            FetchOpDataEvtQuery::Hashes(hashes) => hashes
                .into_iter()
                .map(|hash| {
                    let loc = hash.get_loc().as_loc8();
                    let e: &OpEntry = sb.ops.get(&loc).unwrap();
                    (e.hash.to_owned(), KitsuneOpData::new(vec![loc.as_u8()]))
                })
                .collect(),
            FetchOpDataEvtQuery::Regions(bounds) => bounds
                .into_iter()
                .flat_map(|b| {
                    // dbg!(&b);
                    sb.ops.iter().filter_map(move |(loc, o)| {
                        let contains = b.contains(&DhtLocation::from(*loc), &o.timestamp);
                        contains.then(|| (o.hash.clone(), KitsuneOpData::new(vec![loc.as_u8()])))
                    })
                })
                .collect(),
        })))
    }

    fn handle_sign_network_data(
        &mut self,
        input: SignNetworkDataEvt,
    ) -> KitsuneP2pEventHandlerResult<KitsuneSignature> {
        todo!()
    }
}
