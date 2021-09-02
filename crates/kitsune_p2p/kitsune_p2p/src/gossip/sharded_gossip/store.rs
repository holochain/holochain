//! This module is the ideal interface we would have for the conductor (or other store that kitsune uses).
//! We should update the conductor to match this interface.

use std::collections::HashMap;
use std::{collections::HashSet, ops::Range, sync::Arc};

use crate::event::{
    full_time_window, FetchOpDataEvt, PutAgentInfoSignedEvt, QueryAgentInfoSignedEvt,
    QueryGossipAgentsEvt, QueryOpHashesEvt, TimeWindowMs,
};
use crate::types::event::KitsuneP2pEventSender;
use kitsune_p2p_types::{
    agent_info::AgentInfoSigned,
    bin_types::{KitsuneAgent, KitsuneBinType, KitsuneOpHash, KitsuneSpace},
    dht_arc::{ArcInterval, DhtArcSet},
    KitsuneError, KitsuneResult,
};

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

/// Get all `AgentInfoSigned` for agents in a space.
pub(super) async fn query_agent_info(
    evt_sender: &EventSender,
    space: &Arc<KitsuneSpace>,
    agents: &HashSet<Arc<KitsuneAgent>>,
) -> KitsuneResult<Vec<AgentInfoSigned>> {
    Ok(evt_sender
        .query_agent_info_signed(QueryAgentInfoSignedEvt {
            space: space.clone(),
            agents: Some(agents.clone()),
        })
        .await
        .map_err(KitsuneError::other)?)
}

/// Get the arc intervals for specified agent, paired with their respective agent.
pub(super) async fn local_agent_arcs(
    evt_sender: &EventSender,
    space: &Arc<KitsuneSpace>,
    local_agents: &HashSet<Arc<KitsuneAgent>>,
) -> KitsuneResult<Vec<(Arc<KitsuneAgent>, ArcInterval)>> {
    Ok(query_agent_info(evt_sender, space, local_agents)
        .await?
        .into_iter()
        .map(|info| (info.agent.clone(), info.storage_arc.interval()))
        .collect::<Vec<_>>())
}

/// Get just the arc intervals for specified agents.
pub(super) async fn local_arcs(
    evt_sender: &EventSender,
    space: &Arc<KitsuneSpace>,
    local_agents: &HashSet<Arc<KitsuneAgent>>,
) -> KitsuneResult<Vec<ArcInterval>> {
    Ok(local_agent_arcs(evt_sender, space, local_agents)
        .await?
        .into_iter()
        .map(|(_, arc)| arc)
        .collect())
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

#[derive(Clone)]
pub(super) struct OpHashQuery {
    pub window_ms: TimeWindowMs,
    pub max_ops: usize,
    pub include_limbo: bool,
    pub only_authored: bool,
}

impl Default for OpHashQuery {
    fn default() -> Self {
        Self {
            window_ms: full_time_window(),
            max_ops: usize::MAX,
            include_limbo: false,
            only_authored: false,
        }
    }
}

/// Get all ops for all agents that fall within the specified arcset.
pub(super) async fn all_op_hashes_within_arcset(
    evt_sender: &EventSender,
    space: &Arc<KitsuneSpace>,
    agents: &[(Arc<KitsuneAgent>, ArcInterval)],
    common_arc_set: &DhtArcSet,
    query: OpHashQuery,
) -> KitsuneResult<Option<(Vec<Arc<KitsuneOpHash>>, Range<u64>)>> {
    let agents: Vec<_> = agents
        .iter()
        .map(|(a, i)| {
            // Intersect this agent's arc with the arcset to find the minimal
            // arcset relevant to this agent
            let intersection = common_arc_set.intersection(&DhtArcSet::from_interval(i));
            (a.clone(), intersection)
        })
        .collect();
    Ok(evt_sender
        .query_op_hashes(QueryOpHashesEvt {
            space: space.clone(),
            agents,
            window_ms: query.window_ms,
            max_ops: query.max_ops,
            include_limbo: query.include_limbo,
            only_authored: query.only_authored,
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
        let peer_data = agents
            .iter()
            .filter(|new_info| {
                this_agent_info
                    .storage_arc
                    .contains(new_info.agent.get_loc())
            })
            .map(|i| (**i).clone())
            .collect();
        evt_sender
            .put_agent_info_signed(PutAgentInfoSignedEvt {
                space: space.clone(),
                peer_data,
            })
            .await
            .map_err(KitsuneError::other)?;
    }
    Ok(())
}

pub(super) async fn fetch_ops(
    evt_sender: &EventSender,
    space: &Arc<KitsuneSpace>,
    agents: impl Iterator<Item = &Arc<KitsuneAgent>>,
    op_hashes: Vec<Arc<KitsuneOpHash>>,
    include_limbo: bool,
) -> KitsuneResult<Vec<(Arc<KitsuneOpHash>, Vec<u8>)>> {
    evt_sender
        .fetch_op_data(FetchOpDataEvt {
            space: space.clone(),
            agents: agents.cloned().collect(),
            op_hashes,
            include_limbo,
        })
        .await
        .map_err(KitsuneError::other)
}

/// Put new ops into agents that should hold them.
pub(super) async fn put_ops(
    evt_sender: &EventSender,
    space: &Arc<KitsuneSpace>,
    agent_arcs: Vec<(Arc<KitsuneAgent>, ArcInterval)>,
    ops: Vec<(Arc<KitsuneOpHash>, Vec<u8>)>,
) -> KitsuneResult<()> {
    for (agent, arc) in agent_arcs {
        let ops: Vec<_> = ops
            .iter()
            .filter(|(op_hash, _)| arc.contains(op_hash.get_loc()))
            .cloned()
            .collect();
        if !ops.is_empty() {
            evt_sender
                .gossip(space.clone(), agent.clone(), ops)
                .await
                .map_err(KitsuneError::other)?;
        }
    }

    Ok(())
}

pub(super) async fn put_ops_direct(
    evt_sender: &EventSender,
    space: &Arc<KitsuneSpace>,
    needed_op_hashes: HashMap<Arc<KitsuneAgent>, HashSet<Arc<KitsuneOpHash>>>,
    ops: HashMap<Arc<KitsuneOpHash>, Vec<u8>>,
) -> KitsuneResult<()> {
    for (agent, op_hashes) in needed_op_hashes {
        let ops: Vec<_> = op_hashes
            .iter()
            .filter_map(|op_hash| ops.get(op_hash).map(|v| (op_hash.clone(), v.clone())))
            .collect();
        if !ops.is_empty() {
            evt_sender
                .gossip(space.clone(), agent.clone(), ops)
                .await
                .map_err(KitsuneError::other)?;
        }
    }

    Ok(())
}
