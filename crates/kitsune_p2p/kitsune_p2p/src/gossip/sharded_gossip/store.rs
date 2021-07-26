//! This module is the ideal interface we would have for the conductor (or other store that kitsune uses).
//! We should update the conductor to match this interface.

use std::{collections::HashSet, ops::Range, sync::Arc};

use kitsune_p2p_types::{
    agent_info::AgentInfoSigned,
    bin_types::{KitsuneAgent, KitsuneBinType, KitsuneOpHash, KitsuneSpace},
    dht_arc::{ArcInterval, DhtArcSet},
    KitsuneError, KitsuneResult,
};

use crate::event::{
    FetchOpHashDataEvt, FetchOpHashesForConstraintsEvt, GetAgentInfoSignedEvt,
    PutAgentInfoSignedEvt, QueryAgentInfoSignedEvt, QueryGossipAgentsEvt, TimeWindowMs,
};
use crate::types::event::KitsuneP2pEventSender;

use super::EventSender;

/// Get all agent info signed for a space.
pub(super) async fn all_agent_info(
    evt_sender: &EventSender,
    space: &Arc<KitsuneSpace>,
) -> KitsuneResult<Vec<AgentInfoSigned>> {
    Ok(evt_sender
        .query_agent_info_signed(QueryAgentInfoSignedEvt {
            space: space.clone(),
            agents: None,
        })
        .await
        .map_err(KitsuneError::other)?)
}

/// Get a single agent info.
pub(super) async fn get_agent_info(
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
        .map_err(KitsuneError::other)?)
}

/// Get all `AgentInfoSigned` for agents in a space.
pub(super) async fn query_agent_info(
    evt_sender: &EventSender,
    space: &Arc<KitsuneSpace>,
    agents: &HashSet<Arc<KitsuneAgent>>,
) -> KitsuneResult<Vec<AgentInfoSigned>> {
    Ok(evt_sender
        .query_agent_info_signed(QueryAgentInfoSignedEvt {
            space: space.clone(),
            agents: Some(agents.into_iter().cloned().collect()),
        })
        .await
        .map_err(KitsuneError::other)?)
}

/// Get the arc intervals for all local agents.
pub(super) async fn local_agent_arcs(
    evt_sender: &EventSender,
    space: &Arc<KitsuneSpace>,
    local_agents: &HashSet<Arc<KitsuneAgent>>,
) -> KitsuneResult<Vec<ArcInterval>> {
    Ok(query_agent_info(evt_sender, space, local_agents)
        .await?
        .into_iter()
        .map(|info| info.storage_arc.interval())
        .collect::<Vec<_>>())
}

/// Get `AgentInfoSigned` for all agents within a `DhtArcSet`.
pub(super) async fn agent_info_within_arc_set(
    evt_sender: &EventSender,
    space: &Arc<KitsuneSpace>,
    arc_set: Arc<DhtArcSet>,
) -> KitsuneResult<impl Iterator<Item = AgentInfoSigned>> {
    let set: HashSet<_> = agents_within_arcset(evt_sender, space, arc_set)
        .await?
        .into_iter()
        .map(|(a, _)| a)
        .collect();
    Ok(all_agent_info(evt_sender, space)
        .await?
        .into_iter()
        .filter(move |info| set.contains(info.agent.as_ref())))
}

/// Get agents and their intervals within a `DhtArcSet`.
pub(super) async fn agents_within_arcset(
    evt_sender: &EventSender,
    space: &Arc<KitsuneSpace>,
    arc_set: Arc<DhtArcSet>,
) -> KitsuneResult<Vec<(Arc<KitsuneAgent>, ArcInterval)>> {
    Ok(evt_sender
        .query_gossip_agents(QueryGossipAgentsEvt {
            space: space.clone(),
            agents: None,
            since_ms: 0,
            until_ms: u64::MAX,
            arc_set,
        })
        .await
        .map_err(KitsuneError::other)?)
}

/// Get all ops for all agents that fall within the specified arcset.
pub(super) async fn all_op_hashes_within_arcset(
    evt_sender: &EventSender,
    space: &Arc<KitsuneSpace>,
    agents: &[(Arc<KitsuneAgent>, ArcInterval)],
    common_arc_set: &DhtArcSet,
    window_ms: TimeWindowMs,
    max_ops: usize,
) -> KitsuneResult<Option<(Vec<Arc<KitsuneOpHash>>, Range<u64>)>> {
    let agents: Vec<_> = agents
        .into_iter()
        .map(|(a, i)| {
            // Intersect this agent's arc with the arcset to find the minimal
            // arcset relevant to this agent
            let intersection = common_arc_set.intersection(&DhtArcSet::from_interval(i));
            (a.clone(), intersection)
        })
        .collect();
    Ok(evt_sender
        .fetch_op_hashes_for_constraints(FetchOpHashesForConstraintsEvt {
            space: space.clone(),
            agents,
            window_ms,
            max_ops,
        })
        .await
        .map_err(KitsuneError::other)?)
}

/// Add new agent info to the p2p store.
pub(super) async fn put_agent_info(
    evt_sender: &EventSender,
    space: &Arc<KitsuneSpace>,
    agents_within_common_arc: HashSet<Arc<KitsuneAgent>>,
    agents: &[Arc<AgentInfoSigned>],
) -> KitsuneResult<()> {
    for this_agent_info in all_agent_info(evt_sender, space)
        .await?
        .into_iter()
        .filter(|a| agents_within_common_arc.contains(a.agent.as_ref()))
    {
        for new_info in agents {
            if this_agent_info
                .storage_arc
                .contains(new_info.agent.get_loc())
            {
                // TODO: PERF: Batch this.
                // We need to change this event type to take a list of agent infos
                // as it takes a lock on the db to write new info and is very
                // slow if not batched.
                evt_sender
                    .put_agent_info_signed(PutAgentInfoSignedEvt {
                        space: space.clone(),
                        agent: this_agent_info.agent.clone(),
                        agent_info_signed: (**new_info).clone(),
                    })
                    .await
                    .map_err(KitsuneError::other)?;
            }
        }
    }
    Ok(())
}

pub(super) async fn fetch_ops(
    evt_sender: &EventSender,
    space: &Arc<KitsuneSpace>,
    agents: impl Iterator<Item = &Arc<KitsuneAgent>>,
    op_hashes: Vec<Arc<KitsuneOpHash>>,
) -> KitsuneResult<Vec<(Arc<KitsuneOpHash>, Vec<u8>)>> {
    evt_sender
        .fetch_op_hash_data(FetchOpHashDataEvt {
            space: space.clone(),
            agents: agents.cloned().collect(),
            op_hashes,
        })
        .await
        .map_err(KitsuneError::other)
}

/// Put new ops into agents that should hold them.
pub(super) async fn put_ops(
    evt_sender: &EventSender,
    space: &Arc<KitsuneSpace>,
    agents_within_common_arc: HashSet<Arc<KitsuneAgent>>,
    // maackle: this seems silly, like we should just not have the op hashes in an arc
    ops: Vec<Arc<(Arc<KitsuneOpHash>, Vec<u8>)>>,
) -> KitsuneResult<()> {
    for this_agent_info in all_agent_info(evt_sender, space)
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
                // This requires changing the event type to take multiple ops.
                evt_sender
                    .gossip(
                        space.clone(),
                        this_agent_info.agent.clone(),
                        // FIXME: I don't know which agent this is coming from.
                        // It's wrong to say it's from self.
                        // This is hard to fix. We could just choose any agent
                        // on the remote node or we could divide up each set of
                        // ops into which agent they came from and send across
                        // a hashmap. But this would require iterating through
                        // all the ops multiple times.
                        this_agent_info.agent.clone(),
                        hash.clone(),
                        op.clone(),
                    )
                    .await
                    .map_err(KitsuneError::other)?;
            }
        }
    }

    Ok(())
}
