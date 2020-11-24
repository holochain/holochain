//! This is a temporary quick-hack gossip module for use with the
//! in-memory / full-sync / non-sharded networking module

use crate::agent_store::AgentInfoSigned;
use crate::types::actor::KitsuneP2pResult;
use crate::types::gossip::*;
use crate::*;
use ghost_actor::dependencies::tracing;
use ghost_actor::dependencies::tracing_futures;
use kitsune_p2p_types::dht_arc::DhtArc;
use std::collections::HashSet;
use std::iter::FromIterator;
use std::sync::Arc;

ghost_actor::ghost_chan! {
    /// "Event" requests emitted by the gossip module
    pub chan GossipEvent<crate::KitsuneP2pError> {
        /// get a list of agents we know about
        fn list_neighbor_agents() -> Vec<Arc<KitsuneAgent>>;

        /// fetch op list from/to with constraints
        fn req_op_hashes(
            input: ReqOpHashesEvt,
        ) -> OpHashesAgentHashes;

        /// fetch op data for op hash list
        fn req_op_data(
            input: ReqOpDataEvt
        ) -> OpDataAgentInfo;

        /// we have gossip to forward
        fn gossip_ops(
            from_agent: Arc<KitsuneAgent>,
            to_agent: Arc<KitsuneAgent>,
            ops: Vec<(Arc<KitsuneOpHash>, Vec<u8>)>,
            agents: Vec<AgentInfoSigned>,
        ) -> ();
    }
}

pub type GossipEventReceiver = futures::channel::mpsc::Receiver<GossipEvent>;

/// spawn a gossip module to control gossip for a space
pub fn spawn_gossip_module() -> GossipEventReceiver {
    let (evt_send, evt_recv) = futures::channel::mpsc::channel(10);

    tokio::task::spawn(gossip_loop(evt_send));

    evt_recv
}

#[tracing::instrument(skip(evt_send))]
/// the gossip module is not an actor because we want to pause while
/// awaiting requests - not process requests in parallel.
async fn gossip_loop(
    evt_send: futures::channel::mpsc::Sender<GossipEvent>,
) -> KitsuneP2pResult<()> {
    let mut gossip_data = GossipData::new(evt_send);
    loop {
        gossip_data.take_action().await?;

        tokio::time::delay_for(std::time::Duration::from_millis(10)).await;
    }
}

struct GossipData {
    evt_send: futures::channel::mpsc::Sender<GossipEvent>,
    pending_gossip_list: Vec<(Arc<KitsuneAgent>, Arc<KitsuneAgent>)>,
}

impl GossipData {
    pub fn new(evt_send: futures::channel::mpsc::Sender<GossipEvent>) -> Self {
        Self {
            evt_send,
            pending_gossip_list: Vec::new(),
        }
    }

    pub async fn take_action(&mut self) -> KitsuneP2pResult<()> {
        if self.pending_gossip_list.is_empty() {
            self.fetch_pending_gossip_list().await?;
        } else {
            self.process_next_gossip().await?;
        }
        Ok(())
    }

    async fn fetch_pending_gossip_list(&mut self) -> KitsuneP2pResult<()> {
        let list = self.evt_send.list_neighbor_agents().await?;
        // super naive gossip just processes all combinations
        // also causes duplication because it runs pairs from both sides
        tracing::debug!(?list);
        for a1 in list.iter() {
            for a2 in list.iter() {
                // at the very least, avoid gossiping with ourselves
                if a1 != a2 {
                    self.pending_gossip_list.push((a1.clone(), a2.clone()));
                }
            }
        }
        Ok(())
    }

    async fn process_next_gossip(&mut self) -> KitsuneP2pResult<()> {
        // !is_empty() checked above in take_action
        let (from_agent, to_agent) = self.pending_gossip_list.remove(0);

        // required so from_iters below know the build_hasher type
        type S = HashSet<Arc<KitsuneOpHash>>;
        type A = HashSet<(Arc<KitsuneAgent>, u64)>;

        // we'll just fetch all with no constraints for now
        let (op_hashes_from, agent_info_from) = self
            .evt_send
            .req_op_hashes(ReqOpHashesEvt::new(
                from_agent.clone(), // from not to because we're initiating
                from_agent.clone(),
                DhtArc::new(0, u32::MAX),
                i64::MIN,
                i64::MAX,
            ))
            .await?;
        let op_hashes_from: S = HashSet::from_iter(op_hashes_from);
        let agent_info_from: A = HashSet::from_iter(agent_info_from);

        // we'll just fetch all with no constraints for now
        let (op_hashes_to, agent_info_to) = self
            .evt_send
            .req_op_hashes(ReqOpHashesEvt::new(
                from_agent.clone(),
                to_agent.clone(),
                DhtArc::new(0, u32::MAX),
                i64::MIN,
                i64::MAX,
            ))
            .await?;
        let op_hashes_to: S = HashSet::from_iter(op_hashes_to);
        let agent_info_to: A = HashSet::from_iter(agent_info_to);

        // values that to_agent has, and from_agent needs
        let from_needs = op_hashes_to
            .difference(&op_hashes_from)
            .cloned()
            .collect::<Vec<_>>();
        let from_needs_agents = agent_info_to
            .difference(&agent_info_from)
            .cloned()
            .map(|(ai, _)| ai)
            .collect::<Vec<_>>();

        // values that from_agent has, and to_agent needs
        let to_needs = op_hashes_from
            .difference(&op_hashes_to)
            .cloned()
            .collect::<Vec<_>>();
        let to_needs_agents = agent_info_from
            .difference(&agent_info_to)
            .cloned()
            .map(|(ai, _)| ai)
            .collect::<Vec<_>>();

        // fetch values that to_agent needs from from_agent
        if !to_needs.is_empty() || !to_needs_agents.is_empty() {
            if let Ok((r_ops, r_peers)) = self
                .evt_send
                .req_op_data(ReqOpDataEvt::new(
                    from_agent.clone(), // from not to because we're initiating
                    from_agent.clone(),
                    to_needs,
                    to_needs_agents,
                ))
                .await
            {
                if !r_ops.is_empty() || !r_peers.is_empty() {
                    if let Err(e) = self
                        .evt_send
                        .gossip_ops(from_agent.clone(), to_agent.clone(), r_ops, r_peers)
                        .await
                    {
                        tracing::error!(?e);
                    }
                }
            }
        }

        // fetch values that from_agent needs from to_agent
        if !from_needs.is_empty() || !from_needs_agents.is_empty() {
            if let Ok((r_ops, r_peers)) = self
                .evt_send
                .req_op_data(ReqOpDataEvt::new(
                    from_agent.clone(),
                    to_agent.clone(),
                    from_needs,
                    from_needs_agents,
                ))
                .await
            {
                if !r_ops.is_empty() || !r_peers.is_empty() {
                    if let Err(e) = self
                        .evt_send
                        .gossip_ops(
                            to_agent.clone(), // we fetched from to
                            from_agent.clone(),
                            r_ops,
                            r_peers,
                        )
                        .await
                    {
                        tracing::error!(?e);
                    }
                }
            }
        }

        Ok(())
    }
}
