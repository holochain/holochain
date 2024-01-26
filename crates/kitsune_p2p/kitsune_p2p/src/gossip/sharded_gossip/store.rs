//! This module is the ideal interface we would have for the conductor (or other store that kitsune uses).
//! We should update the conductor to match this interface.

use std::collections::{HashMap, HashSet};
use std::ops::ControlFlow;
use std::sync::Arc;

use crate::event::{
    PutAgentInfoSignedEvt, QueryAgentsEvt, QueryOpHashesEvt, TimeWindow, TimeWindowInclusive,
};
use crate::types::event::KitsuneP2pEventSender;
use crate::{HostApi, HostApiLegacy};
use kitsune_p2p_timestamp::Timestamp;
use kitsune_p2p_types::dht::region_set::RegionSetLtcs;
use kitsune_p2p_types::{
    agent_info::AgentInfoSigned,
    bin_types::{KitsuneAgent, KitsuneOpHash, KitsuneSpace},
    dht_arc::{DhtArc, DhtArcSet},
    KitsuneError, KitsuneResult,
};

use super::ShardedGossipLocal;

/// A short-lived session for agent info. Local agents from Kitsune are combined with the list of agents from the host
/// to allow gossip to access agent info as needed. The session should be regularly refreshed from these sources.
#[derive(Default)]
pub(super) struct AgentInfoSession {
    /// The local agents that have joined a Kitsune space, converted to agent infos by calling the host.
    local_agents: Vec<AgentInfoSigned>,

    /// All the agents for this space.
    ///
    /// This includes both local and remote agents but note that it's possible for local agents to exist in this list but not in the `local_agents` list if the agents
    /// are in the host store but haven't yet joined the Kitsune space.
    all_agents: Vec<AgentInfoSigned>,

    /// Cache of agents whose storage arc is contained in an arc set.
    /// Finding these agents requires a host query so we cache the results because they are used frequently.
    agents_by_arc_set_cache: HashMap<Arc<DhtArcSet>, Vec<AgentInfoSigned>>,
}

impl AgentInfoSession {
    pub(super) fn new(
        local_agents: Vec<AgentInfoSigned>,
        all_agents: Vec<AgentInfoSigned>,
    ) -> Self {
        Self {
            local_agents,
            all_agents,
            agents_by_arc_set_cache: HashMap::new(),
        }
    }

    pub(super) fn get_agents(&self) -> &[AgentInfoSigned] {
        &self.all_agents
    }

    pub(super) fn get_local_agents(&self) -> &[AgentInfoSigned] {
        &self.local_agents
    }

    pub(super) fn get_local_kitsune_agents(&self) -> HashSet<Arc<KitsuneAgent>> {
        self.local_agents
            .iter()
            .map(|info| info.agent.clone())
            .collect()
    }

    pub(super) fn local_agent_arcs(&self) -> Vec<(Arc<KitsuneAgent>, DhtArc)> {
        self.local_agents
            .iter()
            .map(|info| (info.agent.clone(), info.storage_arc))
            .collect()
    }

    // Get the arc intervals for locally joined agents.
    pub(super) fn local_arcs(&self) -> Vec<DhtArc> {
        self.local_agents
            .iter()
            .map(|info| info.storage_arc)
            .collect()
    }

    pub(super) async fn agent_info_within_arc_set(
        &mut self,
        host_api: &HostApiLegacy,
        space: &Arc<KitsuneSpace>,
        arc_set: Arc<DhtArcSet>,
    ) -> KitsuneResult<Vec<AgentInfoSigned>> {
        match self.agents_by_arc_set_cache.entry(arc_set.clone()) {
            std::collections::hash_map::Entry::Occupied(o) => Ok(o.get().clone()),
            std::collections::hash_map::Entry::Vacant(v) => {
                let agents = host_api
                    .legacy
                    .query_agents(QueryAgentsEvt::new(space.clone()).by_arc_set(arc_set))
                    .await
                    .map_err(KitsuneError::other)?;
                v.insert(agents.clone());
                Ok(agents)
            }
        }
    }
}

/// Get all agent info signed for a space.
pub(super) async fn all_agent_info(
    host_api: &HostApiLegacy,
    space: &Arc<KitsuneSpace>,
) -> KitsuneResult<Vec<AgentInfoSigned>> {
    host_api
        .legacy
        .query_agents(QueryAgentsEvt::new(space.clone()))
        .await
        .map_err(KitsuneError::other)
}

/// Get all ops for all agents that fall within the specified arcset.
pub(super) async fn all_op_hashes_within_arcset(
    host_api: &HostApiLegacy,
    space: &Arc<KitsuneSpace>,
    common_arc_set: DhtArcSet,
    window: TimeWindow,
    max_ops: usize,
    include_limbo: bool,
) -> KitsuneResult<Option<(Vec<Arc<KitsuneOpHash>>, TimeWindowInclusive)>> {
    host_api
        .legacy
        .query_op_hashes(QueryOpHashesEvt {
            space: space.clone(),
            arc_set: common_arc_set,
            window,
            max_ops,
            include_limbo,
        })
        .await
        .map_err(KitsuneError::other)
}

/// A chunk of hashes.
pub struct TimeChunk {
    /// The time window they were found in.
    pub window: TimeWindow,
    /// The final hashes position.
    /// Note this is not the same as the window.end
    /// as the window is an exclusive range and
    /// the cursor is purposely set the the last
    /// hashes position because the next hash could
    /// have the same timestamp.
    pub cursor: Timestamp,
    /// The hashes found in this chunk.
    pub hashes: Vec<Arc<KitsuneOpHash>>,
}

/// This query returns a stream of hashes chunked
/// by time window.
///
/// If all the hashes found in the search time window
/// fit into a single chunk then this will return one chunk.
///
/// Otherwise a chunk will be returned with the window for the hashes
/// that fit into a single chunk and the following chunk will attempt to
/// be produced from the remaining time window.
///
/// This process will continue until the time window is small enough that
/// all the hashes fit into the final chunk.
/// The final chunk will always have a time window with an end that matches
/// the end of the search time window.
///
/// If there are no hashes found for a time window then the remaining
/// time window is returned with an empty hashes vector.
/// Due to this fact this stream always returns at least one value because
/// even if there are no hashes the full time window will return with an empty
/// hashes vector.
///
/// This stream is very useful for pulling hash chunks until some limit is reached
/// where the cursor can be saved an a new hash query can be started in the future
/// where the search time window starts from the previous queries cursor.
pub(super) fn hash_chunks_query(
    host_api: HostApiLegacy,
    space: Arc<KitsuneSpace>,
    common_arc_set: DhtArcSet,
    search_time_window: TimeWindow,
    include_limbo: bool,
) -> impl futures::stream::TryStream<Ok = TimeChunk, Error = KitsuneError> + Unpin {
    let f = futures::stream::try_unfold(
        // The stream starts with the full time window and control flow is set to continue.
        (search_time_window, ControlFlow::Continue(())),
        move |(mut search_time_window, control_flow)| {
            let host_api = host_api.clone();
            let space = space.clone();
            let common_arc_set = common_arc_set.clone();
            async move {
                if let ControlFlow::Break(_) = control_flow {
                    // The previous iteration has decided to break the stream.
                    return Ok(None);
                }

                // Run the hash query for the current search time window up to the hashes limit.
                let result = all_op_hashes_within_arcset(
                    &host_api,
                    &space,
                    common_arc_set.clone(),
                    search_time_window.clone(),
                    ShardedGossipLocal::UPPER_HASHES_BOUND,
                    include_limbo,
                )
                .await?;

                let (hashes, found_time_window) = match result {
                    Some(r) => r,
                    None => {
                        // If no hashes were found then return the final time chunk with
                        // an empty hashes vector and break the stream.
                        let chunk = TimeChunk {
                            window: search_time_window.clone(),
                            cursor: search_time_window.end,
                            hashes: Vec::with_capacity(0),
                        };
                        return Ok(Some((chunk, (search_time_window, ControlFlow::Break(())))));
                    }
                };

                let num_found = hashes.len();

                // The found time window is inclusive and the end is the timestamp
                // of the final hash. If this is the final chunk the consumer wants
                // then this is the cursor they should start from in the future.
                let cursor = *found_time_window.end();

                // If we found the upper hashes bound then we are not done.
                if num_found >= ShardedGossipLocal::UPPER_HASHES_BOUND {
                    // The time window is the searches start to the found windows
                    // end.
                    // Because this window needs to be exclusive a micro second (the smallest
                    // unit in our timestamps) is added.
                    let window = search_time_window.start
                        ..found_time_window
                            .end()
                            .saturating_add(&std::time::Duration::from_micros(1));

                    // The search window for the next call is reduced to the timestamp of the final
                    // hash from this call (because multiple hashes can share the same timestamp) to
                    // the end of the search time window.
                    search_time_window = *found_time_window.end()..search_time_window.end;

                    let chunk = TimeChunk {
                        window,
                        cursor,
                        hashes,
                    };

                    // Return this chunk and continue the stream.
                    Ok(Some((
                        chunk,
                        (search_time_window, ControlFlow::Continue(())),
                    )))
                } else {
                    // The remaining hashes fit into this chunk so
                    // this is the final chunk and has a time window equal to
                    // this iterations search window.
                    let chunk = TimeChunk {
                        window: search_time_window.clone(),
                        cursor,
                        hashes,
                    };

                    // Return the final chunk and break the stream.
                    Ok(Some((chunk, (search_time_window, ControlFlow::Break(())))))
                }
            }
        },
    );
    Box::pin(f)
}

pub(super) async fn query_region_set<'a>(
    host_api: HostApi,
    space: Arc<KitsuneSpace>,
    common_arc_set: Arc<DhtArcSet>,
) -> KitsuneResult<RegionSetLtcs> {
    host_api
        .query_region_set(space, common_arc_set)
        .await
        .map_err(KitsuneError::other)
}

/// Add new agent info to the p2p store.
pub(super) async fn put_agent_info(
    host_api: &HostApiLegacy,
    space: &Arc<KitsuneSpace>,
    agents: &[Arc<AgentInfoSigned>],
) -> KitsuneResult<()> {
    let peer_data: Vec<_> = agents.iter().map(|i| (**i).clone()).collect();
    host_api
        .legacy
        .put_agent_info_signed(PutAgentInfoSignedEvt {
            space: space.clone(),
            peer_data,
        })
        .await
        .map_err(KitsuneError::other)?;
    Ok(())
}
