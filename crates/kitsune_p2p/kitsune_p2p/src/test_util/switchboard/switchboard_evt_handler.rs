use std::collections::HashMap;
use std::sync::Arc;

use crate::event::*;
use crate::test_util::switchboard::switchboard::AgentEntry;
use crate::types::event::{KitsuneP2pEvent, KitsuneP2pEventHandler, KitsuneP2pEventHandlerResult};
use kitsune_p2p_types::bin_types::*;
use kitsune_p2p_types::dht_arc::DhtArcSet;
use kitsune_p2p_types::tx2::tx2_utils::Share;
use kitsune_p2p_types::*;

use super::switchboard::{AgentOpEntry, NodeEp, OpEntry, SwitchboardSpace};

type KSpace = Arc<KitsuneSpace>;
type KAgent = Arc<KitsuneAgent>;
type KOpHash = Arc<KitsuneOpHash>;

#[derive(Clone)]
pub struct SwitchboardEventHandler {
    node: NodeEp,
    sb: Share<SwitchboardSpace>,
}

impl SwitchboardEventHandler {
    pub fn new(node: NodeEp, sb: Share<SwitchboardSpace>) -> Self {
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
        dbg!("handle_put_agent_info_signed");
        self.sb.share_mut(|state, _| {
            state.local_agents_for_node(&self.node).extend(
                peer_data
                    .into_iter()
                    .map(|info| (info.agent.get_loc().as_loc8(), AgentEntry::new(info))),
            );
            Ok(())
        })?;
        ok_fut(Ok(()))
    }

    fn handle_get_agent_info_signed(
        &mut self,
        GetAgentInfoSignedEvt { space, agent }: GetAgentInfoSignedEvt,
    ) -> KitsuneP2pEventHandlerResult<Option<crate::types::agent_store::AgentInfoSigned>> {
        dbg!("handle_get_agent_info_signed");
        ok_fut(Ok(self.sb.share_mut(|state, _| {
            Ok(state
                .local_agents_for_node(&self.node)
                .get(&agent.get_loc().as_loc8())
                .map(|e| e.info.to_owned()))
        })?))
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
        let result = self.sb.share_mut(|state, _| {
            let node = &state.nodes.get(&self.node).expect("Node not added");
            let all_agents = node
                .local_agents
                .values()
                .map(|e| &e.info)
                .chain(node.remote_agents.values());
            Ok(if let Some(agents) = agents {
                all_agents
                    .filter(|info| agents.contains(&info.agent))
                    .cloned()
                    .collect()
            } else {
                all_agents.cloned().collect()
            })
        })?;
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
        ok_fut(Ok(self.sb.share_mut(|sb, _| {
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
            Ok(())
        })?))
    }

    fn handle_query_op_hashes(
        &mut self,
        query: QueryOpHashesEvt,
    ) -> KitsuneP2pEventHandlerResult<Option<(Vec<Arc<KitsuneOpHash>>, TimeWindow)>> {
        ok_fut(Ok(self
            .sb
            .share_mut(|sb, _| Ok(dbg!(sb.query_op_hashes(query))))?))
    }

    fn handle_fetch_op_data(
        &mut self,
        FetchOpDataEvt {
            space,
            agents,
            op_hashes,
        }: FetchOpDataEvt,
    ) -> KitsuneP2pEventHandlerResult<Vec<(Arc<KitsuneOpHash>, Vec<u8>)>> {
        ok_fut(Ok(self.sb.share_mut(|sb, _| {
            Ok(op_hashes
                .into_iter()
                .map(|hash| {
                    let e: &OpEntry = sb.ops.get(&hash.get_loc().as_loc8()).unwrap();
                    (e.hash.to_owned(), e.data.to_owned())
                })
                .collect())
        })?))
    }

    fn handle_sign_network_data(
        &mut self,
        input: SignNetworkDataEvt,
    ) -> KitsuneP2pEventHandlerResult<KitsuneSignature> {
        todo!()
    }
}
