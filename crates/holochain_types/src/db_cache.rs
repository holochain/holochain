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
    dht_env: DbRead<DbKindDht>,
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
    pub out_of_order: Vec<u32>,
}

#[derive(Default, Debug, Clone, Copy, PartialEq, Eq)]
/// The state of an agent's activity.
pub struct ActivityBounds {
    /// The highest agent activity header sequence that is already integrated.
    pub integrated: Option<u32>,
    /// The highest consecutive header sequence number that is ready to integrate.
    pub ready_to_integrate: Option<u32>,
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
    fn out(mut self, i: Vec<u32>) -> Self {
        self.out_of_order = i;
        self
    }
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
    pub fn new(dht_env: DbRead<DbKindDht>) -> Self {
        Self {
            dht_env,
            activity: Default::default(),
        }
    }

    /// Lazily initiate the activity cache.
    async fn get_or_try_init(&self) -> DatabaseResult<&ActivityCache> {
        self.activity
            .get_or_try_init(|| {
                let env = self.dht_env.clone();
                async move {
                    let (activity_integrated, mut all_activity) = env
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

                            // Get all agents that have any activity.
                            // This is needed for agents that have activity but it's not integrated or
                            // ready to be integrated yet.
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

                            for author in all_activity_agents {
                                let out_of_order = stmt
                                    .query_map(
                                        named_params! {
                                            ":register_activity": DhtOpType::RegisterAgentActivity,
                                            ":author": author,
                                        },
                                        |row| row.get::<_, u32>(0),
                                    )?
                                    .collect::<rusqlite::Result<Vec<_>>>()?;
                                let state = ActivityState {
                                    out_of_order,
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

                    for state in all_activity.values_mut() {
                        dbg!(&state);
                        update_ready_to_integrate(state, None);
                        dbg!(&state);
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
                    let ready_to_integrate = bounds.ready_to_integrate?;
                    let start = bounds
                        .integrated
                        .map(|i| i + 1)
                        .filter(|i| *i <= ready_to_integrate)
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
            for (author, seq_range) in integrated_activity {
                let prev_bounds = activity.get_mut(author.as_ref());
                new_bounds.integrated = Some(*seq_range.start());
                if !update_activity_check(prev_bounds.as_deref().map(|p| &p.bounds), &new_bounds) {
                    return Err(DbCacheError::ActivityOutOfOrder(
                        prev_bounds.and_then(|p| p.integrated).unwrap_or(0),
                        new_bounds.integrated.unwrap_or(0),
                    ));
                }
                new_bounds.integrated = Some(*seq_range.end());
                match prev_bounds {
                    Some(prev_bounds) => update_activity_inner(prev_bounds, &new_bounds),
                    None => {
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
            if new_bounds.ready_to_integrate.map_or(false, |i| i != 0) {
                activity.insert(
                    Arc::new(agent.clone()),
                    ActivityState {
                        bounds: ActivityBounds {
                            integrated: new_bounds.integrated,
                            ..Default::default()
                        },
                        out_of_order: new_bounds.ready_to_integrate.into_iter().collect(),
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

// If there's already activity marked ready to integrate
// we want to take the maximum of the two.
//
fn update_ready_to_integrate(prev_state: &mut ActivityState, new_ready: Option<u32>) {
    if let Some(new_ready) = new_ready {
        match prev_state {
            ActivityState {
                bounds:
                    ActivityBounds {
                        integrated: None,
                        ready_to_integrate: ready @ None,
                    },
                out_of_order,
            } => {
                // (0) -> Ready(0)
                if new_ready == 0 {
                    *ready = Some(new_ready);
                // (x) -> Out(x) where x > 0
                } else {
                    out_of_order.push(new_ready);
                    out_of_order.sort_unstable();
                }
            }
            // else
            // (Ready(x), a) -> (Ready(x), Out(y) where a != x')
            // (Ready(x), Out(y..), a) -> (Ready(x), Out(y, a)) where a != x'
            ActivityState {
                bounds:
                    ActivityBounds {
                        integrated: _,
                        ready_to_integrate: Some(x),
                    },
                out_of_order,
            } => {
                // (Ready(x), x') -> Ready(x')
                if x.checked_add(1)
                    .map_or(false, |x_prime| x_prime == new_ready)
                {
                    // (Ready(x), Out(x''..y), x') -> Ready(y)
                    // (Ready(x), Out(x''..y, z..), x') -> (Ready(y), Out(z..))
                    if x.checked_add(2)
                        .and_then(|x_prime_prime| {
                            out_of_order.first().map(|first| x_prime_prime == *first)
                        })
                        .unwrap_or(false)
                    {
                        if let Some(y) = find_consecutive(out_of_order) {
                            *x = y;
                        }
                    } else {
                        *x = new_ready;
                    }
                } else {
                    out_of_order.push(new_ready);
                    out_of_order.sort_unstable();
                }
            }
            ActivityState {
                bounds:
                    ActivityBounds {
                        integrated: Some(x),
                        ready_to_integrate: ready @ None,
                    },
                out_of_order,
            } => {
                // (Integrated(x), x') -> (Integrated(x), Ready(x'))
                if x.checked_add(1)
                    .map_or(false, |x_prime| x_prime == new_ready)
                {
                    *ready = Some(new_ready);
                // (Integrated(x), a) -> (Integrated(x), Out(y)) where a != 'x
                } else {
                    out_of_order.push(new_ready);
                    out_of_order.sort_unstable();
                }
            }
        }
    }
    // (Ready(x), Out(x'..y)) -> (Ready(y))
    // (Ready(x), Out(x'..y, z..)) -> (Ready(y), Out(z..))
    match prev_state {
        ActivityState {
            bounds:
                ActivityBounds {
                    ready_to_integrate: Some(x),
                    ..
                },
            out_of_order,
        } => {
            if x.checked_add(1)
                .and_then(|x_prime| out_of_order.first().map(|first| x_prime == *first))
                .unwrap_or(false)
            {
                if let Some(y) = find_consecutive(out_of_order) {
                    *x = y;
                }
            }
        }
        ActivityState {
            bounds:
                ActivityBounds {
                    integrated: Some(x),
                    ready_to_integrate: ready @ None,
                },
            out_of_order,
        } => {
            if x.checked_add(1)
                .and_then(|x_prime| out_of_order.first().map(|first| x_prime == *first))
                .unwrap_or(false)
            {
                if let Some(y) = find_consecutive(out_of_order) {
                    *ready = Some(y);
                }
            }
        }
        ActivityState {
            bounds:
                ActivityBounds {
                    integrated: None,
                    ready_to_integrate: ready @ None,
                },
            out_of_order,
        } => {
            if out_of_order.first().map_or(false, |first| *first == 0) {
                if let Some(y) = find_consecutive(out_of_order) {
                    *ready = Some(y);
                }
            }
        }
    }
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
fn find_consecutive(out_of_order: &mut Vec<u32>) -> Option<u32> {
    if out_of_order.len() == 1 {
        out_of_order.pop()
    } else {
        let last_consecutive_pos = out_of_order
            .iter()
            .zip(out_of_order.iter().skip(1))
            .position(|(n, delta)| {
                n.checked_add(1)
                    .map(|n_prime| n_prime != *delta)
                    .unwrap_or(true)
            });
        match last_consecutive_pos {
            Some(pos) => {
                let r = out_of_order.get(pos).copied();
                // Drop the consecutive seqs.
                drop(out_of_order.drain(..=pos));
                out_of_order.shrink_to_fit();
                r
            }
            None => {
                let r = out_of_order.pop();
                out_of_order.clear();
                r
            }
        }
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
