//! # Database Cache
//! This is an in-memory cache that is used to store the state of the DHT database.
use crate::dht_op::DhtOpType;
use crate::share::RwShare;
use error::*;
use holo_hash::*;
use holochain_sqlite::prelude::*;
use rusqlite::named_params;
use std::collections::HashMap;
use std::ops::RangeInclusive;
use std::sync::Arc;

#[cfg(test)]
mod tests;

#[allow(missing_docs)]
pub mod error;

#[derive(Clone)]
/// This cache allows us to track selected database queries that
/// are too slow to run frequently.
/// The queries are lazily evaluated and cached.
/// Then the cache is updated in memory without needing to
/// go to the database.
pub struct DhtDbQueryCache {
    /// The database this is caching queries for.
    dht_db: DbRead<DbKindDht>,
    /// The cache of agent activity queries.
    activity: Arc<tokio::sync::OnceCell<ActivityCache>>,
}

type ActivityCache = RwShare<HashMap<Arc<AgentPubKey>, ActivityState>>;

#[derive(Default, Debug, Clone, PartialEq, Eq)]
/// The state of an authors activity according to this authority.
pub struct ActivityState {
    /// The bounds of integrated and ready to integrate activity.
    pub bounds: ActivityBounds,
    /// Any activity that is ready to be integrated but is waiting
    /// for one or more upstream chain items to be marked ready before it can
    /// be integrated.
    /// This is an ordered sparse set.
    pub awaiting_deps: Vec<u32>,
}

#[derive(Default, Debug, Clone, Copy, PartialEq, Eq)]
/// The state of an agent's activity.
pub struct ActivityBounds {
    /// The highest agent activity header sequence that is already integrated.
    pub integrated: Option<u32>,
    /// The highest consecutive header sequence number that is ready to integrate.
    pub ready_to_integrate: Option<u32>,
}

impl std::ops::Deref for ActivityState {
    type Target = ActivityBounds;

    fn deref(&self) -> &Self::Target {
        &self.bounds
    }
}

#[cfg(any(test, feature = "test_utils"))]
impl DhtDbQueryCache {
    /// Get the caches internal state for testing.
    pub async fn get_state(&self) -> &ActivityCache {
        self.get_or_try_init().await.unwrap()
    }
}

impl DhtDbQueryCache {
    /// Create a new cache for dht database queries.
    pub fn new(dht_db: DbRead<DbKindDht>) -> Self {
        Self {
            dht_db,
            activity: Default::default(),
        }
    }

    /// Lazily initiate the activity cache.
    async fn get_or_try_init(&self) -> DatabaseResult<&ActivityCache> {
        self.activity
            .get_or_try_init(|| {
                let db = self.dht_db.clone();
                async move {
                    let (activity_integrated, mut all_activity) = db
                        .async_reader(|txn| {
                            // Get the highest integrated sequence number for each agent.
                            let activity_integrated: Vec<(AgentPubKey, u32)> = txn
                            .prepare_cached(
                                holochain_sqlite::sql::sql_cell::ACTIVITY_INTEGRATED_UPPER_BOUND,
                            )?
                            .query_map(
                                named_params! {
                                    ":register_activity": DhtOpType::RegisterAgentActivity,
                                },
                                |row| {
                                    Ok((
                                        row.get::<_, Option<AgentPubKey>>(0)?,
                                        row.get::<_, Option<u32>>(1)?,
                                    ))
                                },
                            )?
                            .filter_map(|r| match r {
                                Ok((a, seq)) => Some(Ok((a?, seq?))),
                                Err(e) => Some(Err(e)),
                            })
                            .collect::<rusqlite::Result<Vec<_>>>()?;

                            // Get all the agents that have activity ready to be integrated.
                            let all_activity_agents: Vec<Arc<AgentPubKey>> = txn
                                .prepare_cached(
                                    holochain_sqlite::sql::sql_cell::ALL_ACTIVITY_AUTHORS,
                                )?
                                .query_map(
                                    named_params! {
                                        ":register_activity": DhtOpType::RegisterAgentActivity,
                                    },
                                    |row| Ok(Arc::new(row.get::<_, AgentPubKey>(0)?)),
                                )?
                                .collect::<rusqlite::Result<Vec<_>>>()?;

                            // Any agent activity that is currently ready to be integrated.
                            let mut any_ready_activity: HashMap<Arc<AgentPubKey>, ActivityState> =
                                HashMap::with_capacity(all_activity_agents.len());
                            let mut stmt = txn.prepare_cached(
                                holochain_sqlite::sql::sql_cell::ALL_READY_ACTIVITY,
                            )?;

                            // For each agent with activity that is ready to be integrated gather all
                            // the chain items and add them to the `awaiting_deps` list.
                            for author in all_activity_agents {
                                let awaiting_deps = stmt
                                    .query_map(
                                        named_params! {
                                            ":register_activity": DhtOpType::RegisterAgentActivity,
                                            ":author": author,
                                        },
                                        |row| row.get::<_, u32>(0),
                                    )?
                                    .collect::<rusqlite::Result<Vec<_>>>()?;
                                let state = ActivityState {
                                    awaiting_deps,
                                    ..Default::default()
                                };
                                any_ready_activity.insert(author, state);
                            }

                            DatabaseResult::Ok((activity_integrated, any_ready_activity))
                        })
                        .await?;

                    // Update the activity with the integrated sequence numbers.
                    for (agent, i) in activity_integrated {
                        let state = all_activity.entry(Arc::new(agent)).or_default();
                        state.bounds.integrated = Some(i);
                    }

                    // Now for each agent we update their activity so that any chain items
                    // that are ready to integrate are moved out of the `awaiting_deps` list.
                    for state in all_activity.values_mut() {
                        update_ready_to_integrate(state, None);
                    }

                    Ok(RwShare::new(all_activity))
                }
            })
            .await
    }

    /// Get any activity that is ready to be integrated.
    /// This returns a range of activity that is ready to be integrated
    /// for each agent.
    pub async fn get_activity_to_integrate(
        &self,
    ) -> DatabaseResult<Vec<(Arc<AgentPubKey>, RangeInclusive<u32>)>> {
        Ok(self.get_or_try_init().await?.share_ref(|activity| {
            activity
                .iter()
                .filter_map(|(agent, ActivityState { bounds, .. })| {
                    // If there is anything ready to integrated then it will be the end of the range.
                    let ready_to_integrate = bounds.ready_to_integrate?;

                    // The start of the range will be one more then the last integrated item or
                    // if there is nothing integrated then the start will be also the ready_to_integrate.
                    // This is why we use an inclusive range.
                    let start = bounds
                        .integrated
                        .and_then(|i| i.checked_add(1))
                        .filter(|i_prime| *i_prime <= ready_to_integrate)
                        .unwrap_or(ready_to_integrate);
                    Some((agent.clone(), start..=ready_to_integrate))
                })
                .collect()
        }))
    }

    /// Is the SourceChain empty for this [`AgentPubKey`]?
    pub async fn is_chain_empty(&self, author: &AgentPubKey) -> DatabaseResult<bool> {
        Ok(self.get_or_try_init().await?.share_ref(|activity| {
            activity
                .get(author)
                .map_or(true, |state| state.bounds.integrated.is_none())
        }))
    }

    /// Mark agent activity as actually integrated.
    pub async fn set_all_activity_to_integrated(
        &self,
        integrated_activity: Vec<(Arc<AgentPubKey>, RangeInclusive<u32>)>,
    ) -> DbCacheResult<()> {
        self.get_or_try_init().await?.share_mut(|activity| {
            let mut new_bounds = ActivityBounds::default();

            // For each authors activity run the activity check then update the activity state.
            for (author, seq_range) in integrated_activity {
                let prev_bounds = activity.get_mut(author.as_ref());

                // Set the new bounds to the start of this range for the check.
                new_bounds.integrated = Some(*seq_range.start());

                // Check that it makes sense to integrate the first activity in this range.
                if !update_activity_check(prev_bounds.as_deref().map(|p| &p.bounds), &new_bounds) {
                    return Err(DbCacheError::ActivityOutOfOrder(
                        prev_bounds.and_then(|p| p.integrated).unwrap_or(0),
                        new_bounds.integrated.unwrap_or(0),
                    ));
                }

                // Because ranges are sequential we know by induction that the last activity makes sense to add.
                // Update the bounds to the end of this range.
                new_bounds.integrated = Some(*seq_range.end());

                // If there is previous bounds then update the bounds.
                match prev_bounds {
                    Some(prev_bounds) => update_activity_inner(prev_bounds, &new_bounds),
                    None => {
                        // Otherwise insert the new state.
                        activity.insert(
                            author,
                            ActivityState {
                                bounds: new_bounds,
                                ..Default::default()
                            },
                        );
                    }
                }
            }
            Ok(())
        })
    }

    /// Set activity to ready to integrate.
    pub async fn set_activity_ready_to_integrate(
        &self,
        agent: &AgentPubKey,
        header_sequence: u32,
    ) -> DbCacheResult<()> {
        self.new_activity_inner(
            agent,
            ActivityBounds {
                ready_to_integrate: Some(header_sequence),
                ..Default::default()
            },
        )
        .await
    }

    /// Set activity to to integrated.
    pub async fn set_activity_to_integrated(
        &self,
        agent: &AgentPubKey,
        header_sequence: u32,
    ) -> DbCacheResult<()> {
        self.new_activity_inner(
            agent,
            ActivityBounds {
                integrated: Some(header_sequence),
                ..Default::default()
            },
        )
        .await
    }

    /// Add an authors activity.
    async fn new_activity_inner(
        &self,
        agent: &AgentPubKey,
        new_bounds: ActivityBounds,
    ) -> DbCacheResult<()> {
        self.get_or_try_init()
            .await?
            .share_mut(|activity| update_activity(activity, agent, &new_bounds))
    }
}

/// Check activity bounds can be added.
fn update_activity_check(
    prev_bounds: Option<&ActivityBounds>,
    new_bounds: &ActivityBounds,
) -> bool {
    prev_is_empty_new_is_zero(prev_bounds, new_bounds)
        && integrated_is_consecutive(prev_bounds, new_bounds)
}

/// Prev integrated is empty and new integrated is empty or set to zero
fn prev_is_empty_new_is_zero(
    prev_bounds: Option<&ActivityBounds>,
    new_bounds: &ActivityBounds,
) -> bool {
    prev_bounds.map_or(false, |p| p.integrated.is_some())
        || new_bounds.integrated.map_or(true, |i| i == 0)
}

/// If there's already activity marked integrated
/// then only + 1 sequence number can be integrated.
fn integrated_is_consecutive(
    prev_bounds: Option<&ActivityBounds>,
    new_bounds: &ActivityBounds,
) -> bool {
    prev_bounds
        .and_then(|p| p.integrated)
        .zip(new_bounds.integrated)
        .map_or(true, |(p, n)| {
            p.checked_add(1).map(|p1| n == p1).unwrap_or(false)
        })
}

/// Updates the activity state of an author with new bounds.
fn update_activity(
    activity: &mut HashMap<Arc<AgentPubKey>, ActivityState>,
    agent: &AgentPubKey,
    new_bounds: &ActivityBounds,
) -> DbCacheResult<()> {
    let prev_state = activity.get_mut(agent);
    if !update_activity_check(prev_state.as_deref().map(|s| &s.bounds), new_bounds) {
        return Err(DbCacheError::ActivityOutOfOrder(
            prev_state.and_then(|p| p.integrated).unwrap_or(0),
            new_bounds.integrated.unwrap_or(0),
        ));
    }
    match prev_state {
        Some(prev_bounds) => update_activity_inner(prev_bounds, new_bounds),
        None => {
            // If the new bounds have `ready_to_integrate` and do not equal zero
            // then they are awaiting dependencies.
            if new_bounds.ready_to_integrate.map_or(false, |i| i != 0) {
                activity.insert(
                    Arc::new(agent.clone()),
                    ActivityState {
                        bounds: ActivityBounds {
                            integrated: new_bounds.integrated,
                            ..Default::default()
                        },
                        awaiting_deps: new_bounds.ready_to_integrate.into_iter().collect(),
                    },
                );
            } else {
                activity.insert(
                    Arc::new(agent.clone()),
                    ActivityState {
                        bounds: *new_bounds,
                        ..Default::default()
                    },
                );
            }
        }
    }
    DbCacheResult::Ok(())
}

fn update_activity_inner(prev_state: &mut ActivityState, new_bounds: &ActivityBounds) {
    if new_bounds.integrated.is_some() {
        prev_state.bounds.integrated = new_bounds.integrated;
    }
    update_ready_to_integrate(prev_state, new_bounds.ready_to_integrate);
}

/// Updates the ready to integrate state of an activity.
/// This function is a bit complex but is heavily tested and maintains the
/// chain activity can only be set to ready if it makes sense to.
fn update_ready_to_integrate(prev_state: &mut ActivityState, new_ready: Option<u32>) {
    // There is a new chain item that is ready for integration.
    if let Some(new_ready) = new_ready {
        match prev_state {
            // Nothing is integrated or currently ready to integrate but there could
            // be other chain items that are awaiting dependencies.
            ActivityState {
                bounds:
                    ActivityBounds {
                        integrated: None,
                        ready_to_integrate: ready @ None,
                    },
                awaiting_deps,
            } => {
                // (0) -> Ready(0)
                //
                // If we have no state and new_ready is zero
                // then the new ready_to_integrate is set to zero.
                if new_ready == 0 {
                    *ready = Some(new_ready);
                // (x) -> Out(x) where x > 0
                //
                // If new_ready is not zero then it is added to awaiting_deps.
                } else {
                    awaiting_deps.push(new_ready);
                    awaiting_deps.sort_unstable();
                }
            }
            // There is existing chain items that are ready to integrate.
            ActivityState {
                bounds:
                    ActivityBounds {
                        integrated: _,
                        ready_to_integrate: Some(x),
                    },
                awaiting_deps,
            } => {
                // (Ready(x), x') -> Ready(x')
                //
                // If ready_to_integrate + 1 == new_ready then we know this
                // new ready is consecutive from the previous ready_to_integrate.
                if x.checked_add(1)
                    .map_or(false, |x_prime| x_prime == new_ready)
                {
                    let check_awaiting_deps =
                        |x_prime_prime| awaiting_deps.first().map(|first| x_prime_prime == *first);
                    // (Ready(x), Out(x''..=y), x') -> Ready(y)
                    // (Ready(x), Out(x''..=y, z..), x') -> (Ready(y), Out(z..))
                    //
                    // If new_ready fills the gap between ready_to_integrate and the
                    // first sequence in awaiting_deps then we make the end of the sequence
                    // the new read_to_integrate.
                    if x.checked_add(2)
                        .and_then(check_awaiting_deps)
                        .unwrap_or(false)
                    {
                        if let Some(y) = find_consecutive(awaiting_deps) {
                            *x = y;
                        }
                    } else {
                        *x = new_ready;
                    }
                } else {
                    // The new ready chain item is not consecutive from the current
                    // ready so we add it to awaiting_deps.
                    awaiting_deps.push(new_ready);
                    awaiting_deps.sort_unstable();
                }
            }
            // There is an existing chain item that is integrated but
            // no currently ready to integrate.
            ActivityState {
                bounds:
                    ActivityBounds {
                        integrated: Some(x),
                        ready_to_integrate: ready @ None,
                    },
                awaiting_deps,
            } => {
                // (Integrated(x), x') -> (Integrated(x), Ready(x'))
                //
                // If the new ready is consecutive from the integrated then we
                // can set the new ready_to_integrate to the new ready.
                if x.checked_add(1)
                    .map_or(false, |x_prime| x_prime == new_ready)
                {
                    *ready = Some(new_ready);
                // (Integrated(x), a) -> (Integrated(x), Out(y)) where a != 'x
                //
                // The new ready is not consecutive from the integrated so we add
                // it to awaiting_deps.
                } else {
                    awaiting_deps.push(new_ready);
                    awaiting_deps.sort_unstable();
                }
            }
        }
    }
    // Now we have updated the ready_to_integrate and awaiting_deps if
    // there was a new_ready we can check if there is now a new consecutive
    // sequence.
    match prev_state {
        // Check if there is a consecutive sequence from ready_to_integrate to awaiting_deps.
        ActivityState {
            bounds:
                ActivityBounds {
                    ready_to_integrate: Some(x),
                    ..
                },
            awaiting_deps,
        } => {
            if x.checked_add(1)
                .and_then(|x_prime| awaiting_deps.first().map(|first| x_prime == *first))
                .unwrap_or(false)
            {
                if let Some(y) = find_consecutive(awaiting_deps) {
                    *x = y;
                }
            }
        }
        // If there is no ready_to_integrate then
        // check if there is a consecutive sequence from integrated to awaiting_deps.
        ActivityState {
            bounds:
                ActivityBounds {
                    integrated: Some(x),
                    ready_to_integrate: ready @ None,
                },
            awaiting_deps,
        } => {
            if x.checked_add(1)
                .and_then(|x_prime| awaiting_deps.first().map(|first| x_prime == *first))
                .unwrap_or(false)
            {
                if let Some(y) = find_consecutive(awaiting_deps) {
                    *ready = Some(y);
                }
            }
        }
        // Check if there is a zero in the awaiting deps.
        // This should not happen but is here for robustness.
        ActivityState {
            bounds:
                ActivityBounds {
                    integrated: None,
                    ready_to_integrate: ready @ None,
                },
            awaiting_deps,
        } => {
            if awaiting_deps.first().map_or(false, |first| *first == 0) {
                if let Some(y) = find_consecutive(awaiting_deps) {
                    *ready = Some(y);
                }
            }
        }
    }

    // Now the ready_to_integrate and awaiting_deps are updated if
    // the integrated is the same as the read_to_integrate then that
    // chain item was integrated so there is no longer a ready_to_integrate.
    if prev_state
        .bounds
        .integrated
        .and_then(|i| prev_state.ready_to_integrate.map(|r| i == r))
        .unwrap_or(false)
    {
        prev_state.bounds.ready_to_integrate = None;
    }
}

// Out(x..y) -> (y)
// Out(x..y, z..)) -> (Out(z..), y)
//
// Take the awaiting dependencies and if there's a sequence from the start
// then remove it and return the end of the sequence.
fn find_consecutive(awaiting_deps: &mut Vec<u32>) -> Option<u32> {
    if awaiting_deps.len() == 1 {
        awaiting_deps.pop()
    } else {
        let last_consecutive_pos = awaiting_deps
            .iter()
            .zip(awaiting_deps.iter().skip(1))
            .position(|(n, delta)| {
                n.checked_add(1)
                    .map(|n_prime| n_prime != *delta)
                    .unwrap_or(true)
            });
        match last_consecutive_pos {
            Some(pos) => {
                let r = awaiting_deps.get(pos).copied();
                // Drop the consecutive seqs.
                drop(awaiting_deps.drain(..=pos));
                awaiting_deps.shrink_to_fit();
                r
            }
            None => {
                let r = awaiting_deps.pop();
                awaiting_deps.clear();
                r
            }
        }
    }
}

#[cfg(test)]
impl ActivityState {
    fn new() -> Self {
        Self::default()
    }
    fn integrated(mut self, i: u32) -> Self {
        self.bounds.integrated = Some(i);
        self
    }
    fn ready(mut self, i: u32) -> Self {
        self.bounds.ready_to_integrate = Some(i);
        self
    }
    fn awaiting(mut self, i: Vec<u32>) -> Self {
        self.awaiting_deps = i;
        self
    }
}

impl From<DbRead<DbKindDht>> for DhtDbQueryCache {
    fn from(db: DbRead<DbKindDht>) -> Self {
        Self::new(db)
    }
}

impl From<DbWrite<DbKindDht>> for DhtDbQueryCache {
    fn from(db: DbWrite<DbKindDht>) -> Self {
        Self::new(db.into())
    }
}
