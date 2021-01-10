//! This is a temporary quick-hack gossip module for use with the
//! in-memory / full-sync / non-sharded networking module

use crate::types::actor::KitsuneP2pResult;
use crate::types::gossip::*;
use crate::*;
use ghost_actor::dependencies::tracing;
use ghost_actor::dependencies::tracing_futures;
use ghost_actor::GhostError;
use kitsune_p2p_types::dht_arc::DhtArc;
use std::collections::HashMap;
use std::collections::HashSet;
use std::iter::FromIterator;
use std::sync::Arc;

ghost_actor::ghost_chan! {
    /// "Event" requests emitted by the gossip module
    pub chan GossipEvent<crate::KitsuneP2pError> {
        /// get a list of agents we know about
        fn list_neighbor_agents() -> ListNeighborAgents;

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
            input: GossipEvt,
        ) -> ();
    }
}

pub type GossipEventReceiver = futures::channel::mpsc::Receiver<GossipEvent>;

/// spawn a gossip module to control gossip for a space
pub fn spawn_gossip_module(config: Arc<KitsuneP2pConfig>) -> GossipEventReceiver {
    let (evt_send, evt_recv) = futures::channel::mpsc::channel(10);

    tokio::task::spawn(gossip_loop(config, evt_send));

    evt_recv
}

#[tracing::instrument(skip(evt_send))]
/// the gossip module is not an actor because we want to pause while
/// awaiting requests - not process requests in parallel.
async fn gossip_loop(
    config: Arc<KitsuneP2pConfig>,
    evt_send: futures::channel::mpsc::Sender<GossipEvent>,
) -> KitsuneP2pResult<()> {
    let mut gossip_data = GossipData::new(evt_send);
    loop {
        match gossip_data.take_action().await {
            Err(KitsuneP2pError::GhostError(GhostError::Disconnected)) => {
                tracing::warn!("Ghost actor is shutting down so gossip loop is exiting");
                return Ok(());
            }
            Err(e) => {
                tracing::warn!(msg = "gossip loop error", ?e);
            }
            Ok(_) => (),
        }

        tokio::time::delay_for(std::time::Duration::from_millis(
            config.tuning_params.gossip_loop_iteration_delay_ms as u64,
        ))
        .await;
    }
}

struct GossipData {
    evt_send: futures::channel::mpsc::Sender<GossipEvent>,
    pending_gossip_list: Vec<(Arc<KitsuneAgent>, Arc<KitsuneAgent>)>,
    last_counts: HashMap<Arc<KitsuneAgent>, (u64, u64)>,
}

impl GossipData {
    pub fn new(evt_send: futures::channel::mpsc::Sender<GossipEvent>) -> Self {
        Self {
            evt_send,
            pending_gossip_list: Vec::new(),
            last_counts: HashMap::new(),
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
        let (local_agents, remote_agents) = self.evt_send.list_neighbor_agents().await?;
        // super naive gossip just processes all combinations
        // also causes duplication because it runs pairs from both sides
        for (i, a1) in local_agents.iter().enumerate() {
            for a2 in local_agents.iter().skip(i) {
                // at the very least, avoid gossiping with ourselves
                if a1 != a2 {
                    self.pending_gossip_list.push((a1.clone(), a2.clone()));
                }
            }
            for a2 in remote_agents.iter() {
                self.pending_gossip_list.push((a1.clone(), a2.clone()));
            }
        }
        Ok(())
    }

    #[tracing::instrument(skip(self))]
    async fn process_next_gossip(&mut self) -> KitsuneP2pResult<()> {
        // !is_empty() checked above in take_action
        let (from_agent, to_agent) = self.pending_gossip_list.remove(0);
        let span = tracing::debug_span!("next_gossip", ?from_agent, ?to_agent);

        // Get the last count for this interaction
        let last_count = self.last_counts.entry(to_agent.clone()).or_insert((0, 0));

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
                Default::default(), // This is ignored because requesting from self
            ))
            .await?;
        let op_hashes_from = match op_hashes_from {
            OpConsistency::Variance(h) => h,
            // Not currently used
            OpConsistency::Consistent => {
                unreachable!("We don't track consistency of hashes for local requests")
            }
        };
        let op_count = if last_count.0 == op_hashes_from.len() as u64 {
            // We have nothing new for them but
            // they might still have something new
            // for us.
            OpCount::Consistent(last_count.1)
        } else {
            // We have new gossip for them so
            // they need to tell us what they have
            // so we can compute the difference.
            OpCount::Variance
        };
        last_count.0 = op_hashes_from.len() as u64;

        let op_hashes_from: S = HashSet::from_iter(op_hashes_from);
        let agent_info_from: A = HashSet::from_iter(agent_info_from);
        span.in_scope(|| {
            tracing::debug!(from_has_len = ?op_hashes_from.len());
        });

        // we'll just fetch all with no constraints for now
        let (op_hashes_to, agent_info_to) = self
            .evt_send
            .req_op_hashes(ReqOpHashesEvt::new(
                from_agent.clone(),
                to_agent.clone(),
                DhtArc::new(0, u32::MAX),
                i64::MIN,
                i64::MAX,
                op_count,
            ))
            .await?;
        let op_hashes_to = match op_hashes_to {
            OpConsistency::Variance(h) => {
                last_count.1 = h.len() as u64;
                h
            }
            // There's no new gossip from us or them
            // so our job is done.
            OpConsistency::Consistent => {
                return Ok(());
            }
        };
        let op_hashes_to: S = HashSet::from_iter(op_hashes_to);
        let agent_info_to: A = HashSet::from_iter(agent_info_to);
        span.in_scope(|| {
            tracing::debug!(to_has_len = ?op_hashes_to.len());
        });

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
        span.in_scope(|| {
            tracing::debug!(?from_needs_agents);
            tracing::debug!(from_needs_len = ?from_needs.len());
        });

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
        span.in_scope(|| {
            tracing::debug!(?to_needs_agents);
            tracing::debug!(to_needs_len = ?to_needs.len());
        });

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
                        .gossip_ops(GossipEvt::new(
                            from_agent.clone(),
                            to_agent.clone(),
                            r_ops,
                            r_peers,
                        ))
                        .await
                    {
                        span.in_scope(|| {
                            tracing::error!(gossip_failed_to_send = ?e, ?to_agent);
                        });
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
                        .gossip_ops(GossipEvt::new(
                            to_agent.clone(), // we fetched from to
                            from_agent.clone(),
                            r_ops,
                            r_peers,
                        ))
                        .await
                    {
                        span.in_scope(|| {
                            tracing::error!(gossip_failed_to_get_from = ?e, ?to_agent);
                        });
                    }
                }
            }
        }

        Ok(())
    }
}
