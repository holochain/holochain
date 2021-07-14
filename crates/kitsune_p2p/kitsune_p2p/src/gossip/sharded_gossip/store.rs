//! This module is the ideal interface we would have for the conductor (or other store that kitsune uses).
//! We should update the conductor to match this interface.

use std::{collections::HashSet, sync::Arc};

use kitsune_p2p_types::{
    agent_info::AgentInfoSigned,
    bin_types::{KitsuneAgent, KitsuneBinType, KitsuneOpHash, KitsuneSpace},
    dht_arc::{ArcInterval, DhtArcSet},
    KitsuneResult,
};

use crate::event::{
    FetchOpHashesForConstraintsEvt, GetAgentInfoSignedEvt, PutAgentInfoSignedEvt,
    QueryAgentInfoSignedEvt, QueryGossipAgentsEvt,
};
use crate::types::event::KitsuneP2pEventSender;

/// Get all agent info signed for a space.
pub(super) async fn all_agent_info<EventSender: KitsuneP2pEventSender>(
    evt_sender: &EventSender,
    space: &Arc<KitsuneSpace>,
    agent: &Arc<KitsuneAgent>,
) -> KitsuneResult<Vec<AgentInfoSigned>> {
    Ok(evt_sender
        .query_agent_info_signed(QueryAgentInfoSignedEvt {
            space: space.clone(),
            agent: agent.clone(),
        })
        .await
        // TODO: Handle error.
        .unwrap())
}

/// Get a single agent info.
pub(super) async fn get_agent_info<EventSender: KitsuneP2pEventSender>(
    evt_sender: &EventSender,
    space: &Arc<KitsuneSpace>,
    agent: &Arc<KitsuneAgent>,
) -> KitsuneResult<Option<AgentInfoSigned>> {
    Ok(evt_sender
        .get_agent_info_signed(GetAgentInfoSignedEvt {
            space: space.clone(),
            agent: agent.clone(),
        })
        .await
        // TODO: Handle error.
        .unwrap())
}

/// Get all `AgentInfoSigned` for local agents in a space.
pub(super) async fn local_agent_info<'iter, EventSender: KitsuneP2pEventSender>(
    evt_sender: &EventSender,
    space: &Arc<KitsuneSpace>,
    agent: &Arc<KitsuneAgent>,
    local_agents: &'iter HashSet<Arc<KitsuneAgent>>,
) -> KitsuneResult<impl Iterator<Item = AgentInfoSigned> + 'iter> {
    Ok(all_agent_info(evt_sender, space, agent)
        .await?
        .into_iter()
        .filter(move |info| local_agents.contains(info.agent.as_ref())))
}

/// Get the arc intervals for all local agents.
pub(super) async fn local_agent_arcs<EventSender: KitsuneP2pEventSender>(
    evt_sender: &EventSender,
    space: &Arc<KitsuneSpace>,
    local_agents: &HashSet<Arc<KitsuneAgent>>,
    agent: &Arc<KitsuneAgent>,
) -> KitsuneResult<Vec<ArcInterval>> {
    Ok(local_agent_info(evt_sender, space, agent, local_agents)
        .await?
        .map(|info| info.storage_arc.interval())
        .collect::<Vec<_>>())
}

/// Get `AgentInfoSigned` for all agents within a `DhtArcSet`.
pub(super) async fn agent_info_within_arc_set<EventSender: KitsuneP2pEventSender>(
    evt_sender: &EventSender,
    space: &Arc<KitsuneSpace>,
    agent: &Arc<KitsuneAgent>,
    arc_set: Arc<DhtArcSet>,
    since_ms: u64,
    until_ms: u64,
) -> KitsuneResult<impl Iterator<Item = AgentInfoSigned>> {
    let set: HashSet<_> =
        agents_within_arcset(evt_sender, space, agent, arc_set, since_ms, until_ms)
            .await?
            .into_iter()
            .map(|(a, _)| a)
            .collect();
    Ok(all_agent_info(evt_sender, space, agent)
        .await?
        .into_iter()
        .filter(move |info| set.contains(info.agent.as_ref())))
}

/// Get agents and their intervals within a `DhtArcSet`.
pub(super) async fn agents_within_arcset<EventSender: KitsuneP2pEventSender>(
    evt_sender: &EventSender,
    space: &Arc<KitsuneSpace>,
    agent: &Arc<KitsuneAgent>,
    arc_set: Arc<DhtArcSet>,
    since_ms: u64,
    until_ms: u64,
) -> KitsuneResult<Vec<(Arc<KitsuneAgent>, ArcInterval)>> {
    Ok(evt_sender
        .query_gossip_agents(QueryGossipAgentsEvt {
            space: space.clone(),
            agent: agent.clone(),
            since_ms,
            until_ms,
            arc_set,
        })
        .await
        // TODO: Handle error.
        .unwrap())
}

/// Get all ops that are in the intersections
/// between an agents interval and the common
/// arc set.
pub(super) async fn ops_within_common_set<EventSender: KitsuneP2pEventSender>(
    evt_sender: &EventSender,
    space: &Arc<KitsuneSpace>,
    agent: &Arc<KitsuneAgent>,
    interval: &ArcInterval,
    common_arc_set: &Arc<DhtArcSet>,
    since_utc_epoch_s: i64,
    until_utc_epoch_s: i64,
) -> KitsuneResult<Vec<Arc<KitsuneOpHash>>> {
    let mut within_common_arc = Vec::new();
    let intersection = common_arc_set.intersection(&interval.clone().into());
    let intervals = intersection.intervals();
    for interval in intervals {
        let hashes = evt_sender
            .fetch_op_hashes_for_constraints(FetchOpHashesForConstraintsEvt {
                space: space.clone(),
                agent: agent.clone(),
                dht_arc: interval.into(),
                since_utc_epoch_s,
                until_utc_epoch_s,
            })
            .await
            // TODO: Handle Error
            .unwrap();
        within_common_arc.extend(hashes);
    }
    Ok(within_common_arc)
}

/// Get all ops for all agents intersections with
/// the common arc set.
pub(super) async fn all_ops_within_common_set<EventSender: KitsuneP2pEventSender>(
    evt_sender: &EventSender,
    space: &Arc<KitsuneSpace>,
    agents: &Vec<(Arc<KitsuneAgent>, ArcInterval)>,
    common_arc_set: &Arc<DhtArcSet>,
    since_utc_epoch_s: i64,
    until_utc_epoch_s: i64,
) -> KitsuneResult<Vec<Arc<KitsuneOpHash>>> {
    let mut missing_hashes = Vec::new();
    for (agent, interval) in agents {
        let hashes = ops_within_common_set(
            evt_sender,
            &space,
            &agent,
            &interval,
            &common_arc_set,
            since_utc_epoch_s,
            until_utc_epoch_s,
        )
        .await?;
        missing_hashes.extend(hashes);
    }
    Ok(missing_hashes)
}

/// Add new agent info to the p2p store.
pub(super) async fn put_agent_info<EventSender: KitsuneP2pEventSender>(
    evt_sender: &EventSender,
    space: &Arc<KitsuneSpace>,
    agents_within_common_arc: HashSet<Arc<KitsuneAgent>>,
    agents: Vec<Arc<AgentInfoSigned>>,
) -> KitsuneResult<()> {
    if let Some(agent) = agents_within_common_arc.iter().next() {
        for this_agent_info in all_agent_info(evt_sender, space, agent)
            .await?
            .into_iter()
            .filter(|a| agents_within_common_arc.contains(a.agent.as_ref()))
        {
            for new_info in &agents {
                if this_agent_info
                    .storage_arc
                    .contains(new_info.agent.get_loc())
                {
                    // TODO; PERF: Batch this.
                    evt_sender
                        .put_agent_info_signed(PutAgentInfoSignedEvt {
                            space: space.clone(),
                            agent: this_agent_info.agent.clone(),
                            agent_info_signed: (**new_info).clone(),
                        })
                        .await
                        // TODO: Handle Error
                        .unwrap();
                }
            }
        }
    }
    Ok(())
}

/// Put new ops into agents that should hold them.
pub(super) async fn put_ops<EventSender: KitsuneP2pEventSender>(
    evt_sender: &EventSender,
    space: &Arc<KitsuneSpace>,
    agents_within_common_arc: HashSet<Arc<KitsuneAgent>>,
    ops: Vec<Arc<(Arc<KitsuneOpHash>, Vec<u8>)>>,
) -> KitsuneResult<()> {
    if let Some(agent) = agents_within_common_arc.iter().next() {
        for this_agent_info in all_agent_info(evt_sender, space, agent)
            .await?
            .into_iter()
            .filter(|a| agents_within_common_arc.contains(a.agent.as_ref()))
        {
            for data in &ops {
                let hash = &data.0;
                let op = &data.1;
                if this_agent_info.storage_arc.contains(hash.get_loc()) {
                    // FIXME: This absolutely should be batched. Sending one op
                    // at a time to the conductor is very slow.
                    evt_sender
                        .gossip(
                            space.clone(),
                            this_agent_info.agent.clone(),
                            // FIXME: I don't know which agent this is coming from.
                            // It's wrong to say it's from self.
                            this_agent_info.agent.clone(),
                            hash.clone(),
                            op.clone(),
                        )
                        .await
                        // TODO: Handle Error
                        .unwrap();
                }
            }
        }
    }
    Ok(())
}
