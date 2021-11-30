//! Gossip event handler which uses `SwitchboardState` as its underlying persisted store.

#![allow(clippy::unit_arg)]

use std::sync::Arc;

use crate::event::*;
use crate::types::event::{KitsuneP2pEvent, KitsuneP2pEventHandler, KitsuneP2pEventHandlerResult};
use kitsune_p2p_types::bin_types::*;
use kitsune_p2p_types::*;

use super::switchboard_state::{AgentOpEntry, NodeEp, OpEntry, Switchboard};

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

    fn handle_get_agent_info_signed(
        &mut self,
        GetAgentInfoSignedEvt { space, agent }: GetAgentInfoSignedEvt,
    ) -> KitsuneP2pEventHandlerResult<Option<crate::types::agent_store::AgentInfoSigned>> {
        ok_fut(Ok(self.sb.share(|state| {
            let node = state.nodes.get_mut(&self.node).unwrap();
            let loc = agent.get_loc().as_loc8();
            node.local_agents
                .get(&loc)
                .map(|e| e.info.to_owned())
                .or_else(|| node.remote_agents.get(&loc).cloned())
        })))
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
    ) -> KitsuneP2pEventHandlerResult<kitsune_p2p_types::dht_arc::PeerViewAlpha> {
        todo!()
    }

    fn handle_put_metric_datum(&mut self, datum: MetricDatum) -> KitsuneP2pEventHandlerResult<()> {
        todo!()
    }

    fn handle_query_metrics(
        &mut self,
        query: MetricQuery,
    ) -> KitsuneP2pEventHandlerResult<MetricQueryAnswer> {
        todo!()
    }

    fn handle_call(
        &mut self,
        space: Arc<KitsuneSpace>,
        to_agent: Arc<KitsuneAgent>,
        from_agent: Arc<KitsuneAgent>,
        payload: Vec<u8>,
    ) -> KitsuneP2pEventHandlerResult<Vec<u8>> {
        todo!()
    }

    fn handle_notify(
        &mut self,
        space: Arc<KitsuneSpace>,
        to_agent: Arc<KitsuneAgent>,
        from_agent: Arc<KitsuneAgent>,
        payload: Vec<u8>,
    ) -> KitsuneP2pEventHandlerResult<()> {
        todo!()
    }

    fn handle_gossip(
        &mut self,
        space: Arc<KitsuneSpace>,
        to_agent: Arc<KitsuneAgent>,
        ops: Vec<(Arc<KitsuneOpHash>, Vec<u8>)>,
    ) -> KitsuneP2pEventHandlerResult<()> {
        ok_fut(Ok(self.sb.share(|sb| {
            let agent = sb
                .node_for_local_agent_hash_mut(&*to_agent)
                .unwrap()
                .local_agent_by_hash_mut(&*to_agent)
                .unwrap();
            for (hash, op_data) in ops {
                let loc8 = hash.get_loc().as_loc8();
                // TODO: allow setting integration status
                agent.ops.insert(
                    loc8,
                    AgentOpEntry {
                        is_integrated: true,
                    },
                );
            }
        })))
    }

    fn handle_query_op_hashes(
        &mut self,
        query: QueryOpHashesEvt,
    ) -> KitsuneP2pEventHandlerResult<Option<(Vec<Arc<KitsuneOpHash>>, TimeWindow)>> {
        ok_fut(Ok(self.sb.share(|sb| sb.query_op_hashes(query))))
    }

    fn handle_fetch_op_data(
        &mut self,
        FetchOpDataEvt {
            space,
            agents,
            op_hashes,
        }: FetchOpDataEvt,
    ) -> KitsuneP2pEventHandlerResult<Vec<(Arc<KitsuneOpHash>, Vec<u8>)>> {
        ok_fut(Ok(self.sb.share(|sb| {
            op_hashes
                .into_iter()
                .map(|hash| {
                    let e: &OpEntry = sb.ops.get(&hash.get_loc().as_loc8()).unwrap();
                    (e.hash.to_owned(), e.data.to_owned())
                })
                .collect()
        })))
    }

    fn handle_sign_network_data(
        &mut self,
        input: SignNetworkDataEvt,
    ) -> KitsuneP2pEventHandlerResult<KitsuneSignature> {
        todo!()
    }
}
