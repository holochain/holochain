//! Gossip event handler which uses `SwitchboardState` as its underlying persisted store.

use std::sync::Arc;

use crate::event::*;
use crate::test_util::switchboard::switchboard_state::AgentEntry;
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
pub struct SwitchboardEvtHandler {
    node: NodeEp,
    sb: Switchboard,
}

impl SwitchboardEvtHandler {
    /// Constructor
    pub fn new(node: NodeEp, sb: Switchboard) -> Self {
        Self { node, sb }
    }
}

impl ghost_actor::GhostHandler<KitsuneP2pEvent> for SwitchboardEvtHandler {}
impl ghost_actor::GhostControlHandler for SwitchboardEvtHandler {}

#[allow(warnings)]
impl KitsuneP2pEventHandler for SwitchboardEvtHandler {
    fn handle_put_agent_info_signed(
        &mut self,
        PutAgentInfoSignedEvt { space, peer_data }: PutAgentInfoSignedEvt,
    ) -> KitsuneP2pEventHandlerResult<()> {
        dbg!("handle_put_agent_info_signed");
        self.sb.share(|state| {
            state.local_agents_for_node(&self.node).extend(
                peer_data
                    .into_iter()
                    .map(|info| (info.agent.get_loc().as_loc8(), AgentEntry::new(info))),
            );
        });
        ok_fut(Ok(()))
    }

    fn handle_get_agent_info_signed(
        &mut self,
        GetAgentInfoSignedEvt { space, agent }: GetAgentInfoSignedEvt,
    ) -> KitsuneP2pEventHandlerResult<Option<crate::types::agent_store::AgentInfoSigned>> {
        dbg!("handle_get_agent_info_signed");
        ok_fut(Ok(self.sb.share(|state| {
            state
                .local_agents_for_node(&self.node)
                .get(&agent.get_loc().as_loc8())
                .map(|e| e.info.to_owned())
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
            let all_agents = node
                .local_agents
                .values()
                .map(|e| &e.info)
                .chain(node.remote_agents.values());
            if let Some(agents) = agents {
                all_agents
                    .filter(|info| agents.contains(&info.agent))
                    .cloned()
                    .collect()
            } else {
                all_agents.cloned().collect()
            }
        });
        ok_fut(Ok(result))
    }

    fn handle_query_peer_density(
        &mut self,
        space: Arc<KitsuneSpace>,
        dht_arc: kitsune_p2p_types::dht_arc::DhtArc,
    ) -> KitsuneP2pEventHandlerResult<kitsune_p2p_types::dht_arc::PeerDensity> {
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
