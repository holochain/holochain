//! This module is the ideal interface we would have for the conductor (or other store that kitsune uses).
//! We should update the conductor to match this interface.

use std::{collections::HashSet, sync::Arc};

use crate::event::{
    FetchOpDataEvt, PutAgentInfoSignedEvt, QueryAgentsEvt, QueryOpHashesEvt, TimeWindow,
};
use crate::types::event::KitsuneP2pEventSender;
use kitsune_p2p_types::{
    agent_info::AgentInfoSigned,
    bin_types::{KitsuneAgent, KitsuneOpHash, KitsuneSpace},
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
        .query_agents(QueryAgentsEvt::new(space.clone()))
        .await
        .map_err(KitsuneError::other)?)
}

/// Get all `AgentInfoSigned` for agents in a space.
pub(super) async fn query_agent_info(
    evt_sender: &EventSender,
    space: &Arc<KitsuneSpace>,
    agents: &HashSet<Arc<KitsuneAgent>>,
) -> KitsuneResult<Vec<AgentInfoSigned>> {
    let query = QueryAgentsEvt::new(space.clone()).by_agents(agents.clone());
    Ok(evt_sender
        .query_agents(query)
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
        .query_agents(QueryAgentsEvt::new(space.clone()).by_arc_set(arc_set))
        .await
        .map_err(KitsuneError::other)?
        .iter()
        .map(AgentInfoSigned::to_agent_arc)
        .collect())
}

/// Get all ops for all agents that fall within the specified arcset.
pub(super) async fn all_op_hashes_within_arcset(
    evt_sender: &EventSender,
    space: &Arc<KitsuneSpace>,
    agents: &[(Arc<KitsuneAgent>, ArcInterval)],
    common_arc_set: &DhtArcSet,
    window: TimeWindow,
    max_ops: usize,
    include_limbo: bool,
) -> KitsuneResult<Option<(Vec<Arc<KitsuneOpHash>>, TimeWindow)>> {
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
            window,
            max_ops,
            include_limbo,
        })
        .await
        .map_err(KitsuneError::other)?)
}

/// Add new agent info to the p2p store.
pub(super) async fn put_agent_info(
    evt_sender: &EventSender,
    space: &Arc<KitsuneSpace>,
    agents: &[Arc<AgentInfoSigned>],
) -> KitsuneResult<()> {
    let peer_data = agents.iter().map(|i| (**i).clone()).collect();
    evt_sender
        .put_agent_info_signed(PutAgentInfoSignedEvt {
            space: space.clone(),
            peer_data,
        })
        .await
        .map_err(KitsuneError::other)?;
    Ok(())
}

pub(super) async fn fetch_ops(
    evt_sender: &EventSender,
    space: &Arc<KitsuneSpace>,
    agents: impl Iterator<Item = &Arc<KitsuneAgent>>,
    op_hashes: Vec<Arc<KitsuneOpHash>>,
) -> KitsuneResult<Vec<(Arc<KitsuneOpHash>, Vec<u8>)>> {
    evt_sender
        .fetch_op_data(FetchOpDataEvt {
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
    agent_arcs: Vec<(Arc<KitsuneAgent>, ArcInterval)>,
    ops: Vec<(Arc<KitsuneOpHash>, Vec<u8>)>,
) -> KitsuneResult<()> {
    // If there's only a single agent in the space we
    // can avoid cloning the ops which could be large.
    if agent_arcs.len() == 1 {
        let (_agent, arc) = agent_arcs
            .into_iter()
            .next()
            .expect("Can't be none due to len check");
        if arc.is_empty() {
            return Ok(());
        }
        evt_sender
            .gossip(space.clone(), ops)
            .await
            .map_err(KitsuneError::other)?;
    } else {
        for (_agent, arc) in agent_arcs {
            if !arc.is_empty() {
                evt_sender
                    .gossip(space.clone(), ops.clone())
                    .await
                    .map_err(KitsuneError::other)?;
            }
        }
    }

    Ok(())
}
