//! Gossip event handler which uses `SwitchboardState` as its underlying persisted store.

#![allow(clippy::unit_arg)]

use std::sync::Arc;

use crate::event::*;
use crate::types::event::{KitsuneP2pEvent, KitsuneP2pEventHandler, KitsuneP2pEventHandlerResult};
use kitsune_p2p_types::bin_types::*;
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
    ) -> KitsuneP2pEventHandlerResult<kitsune_p2p_types::dht_arc::PeerViewBeta> {
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
                let loc = (op.0[0] as u8 as i8).into();

                // TODO: allow setting integration status
                node.ops.insert(
                    loc,
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
        FetchOpDataEvt { space, op_hashes }: FetchOpDataEvt,
    ) -> KitsuneP2pEventHandlerResult<Vec<(Arc<KitsuneOpHash>, KOp)>> {
        ok_fut(Ok(self.sb.share(|sb| {
            op_hashes
                .into_iter()
                .map(|hash| {
                    let loc = hash.get_loc().as_loc8();
                    let e: &OpEntry = sb.ops.get(&loc).unwrap();
                    (e.hash.to_owned(), KitsuneOpData::new(vec![loc.as_u8()]))
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
