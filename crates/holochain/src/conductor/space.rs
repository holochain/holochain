//! This module contains data and functions for running operations
//! at the level of a [`DnaHash`] space.
//! Multiple [`Cell`]'s could share the same space.

use std::{collections::HashMap, ops::RangeInclusive, sync::Arc};

use holo_hash::{AgentPubKey, DhtOpHash, DnaHash};
use holochain_conductor_api::conductor::EnvironmentRootPath;
use holochain_p2p::dht_arc::{ArcInterval, DhtArcSet};
use holochain_sqlite::{
    conn::{DbSyncLevel, DbSyncStrategy},
    db::{DbKindAuthored, DbKindCache, DbKindDht, DbRead, DbWrite, ReadAccess},
    prelude::DatabaseResult,
};
use holochain_state::prelude::{from_blob, StateQueryResult};
use holochain_types::dht_op::{DhtOp, DhtOpType};
use holochain_zome_types::{Entry, EntryVisibility, SignedHeader, Timestamp};
use kitsune_p2p::event::{TimeWindow, TimeWindowInclusive};
use rusqlite::named_params;
use tracing::instrument;

use crate::core::{
    queue_consumer::QueueConsumerMap,
    workflow::{
        countersigning_workflow::{incoming_countersigning, CountersigningWorkspace},
        error::{WorkflowError, WorkflowResult},
        incoming_dht_ops_workflow::{
            incoming_dht_ops_workflow, IncomingOpHashes, IncomingOpsBatch,
        },
    },
};

use super::{conductor::RwShare, error::ConductorResult};
use std::convert::TryInto;

#[cfg(test)]
mod cache_tests;

#[derive(Clone)]
/// This is the set of all current
/// [`DnaHash`] spaces for all cells
/// installed on this conductor.
pub struct Spaces {
    map: RwShare<HashMap<DnaHash, Space>>,
    root_env_dir: Arc<EnvironmentRootPath>,
    db_sync_level: DbSyncStrategy,
    /// The map of running queue consumer workflows.
    queue_consumer_map: QueueConsumerMap,
}

#[derive(Clone)]
/// This is the set of data required at the
/// [`DnaHash`] space level.
/// All cells in the same [`DnaHash`] space
/// will share these.
pub struct Space {
    /// The dna hash for this space.
    pub dna_hash: Arc<DnaHash>,

    /// The caches databases. These are shared across cells.
    /// There is one per unique Dna.
    pub cache: DbWrite<DbKindCache>,

    /// The authored databases. These are shared across cells.
    /// There is one per unique Dna.
    pub authored_env: DbWrite<DbKindAuthored>,

    /// The dht databases. These are shared across cells.
    /// There is one per unique Dna.
    pub dht_env: DbWrite<DbKindDht>,

    /// A cache for slow database queries.
    pub dht_query_cache: DhtDbQueryCache,

    /// Countersigning workspace that is shared across this cell.
    pub countersigning_workspace: CountersigningWorkspace,

    /// Incoming op hashes that are queued for processing.
    pub incoming_op_hashes: IncomingOpHashes,

    /// Incoming ops batch for this space.
    pub incoming_ops_batch: IncomingOpsBatch,
}

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

#[derive(Default, Debug, Clone, Copy)]
/// The state of an agent's activity.
struct ActivityBounds {
    /// The highest agent activity header sequence that is already integrated.
    integrated: Option<u32>,
    /// The highest consecutive header sequence number that is ready to integrate.
    ready_to_integrate: Option<u32>,
}

#[derive(Default, Debug, Clone)]
struct ActivityState {
    bounds: ActivityBounds,
    out_of_order: Vec<u32>,
}

impl std::ops::Deref for ActivityState {
    type Target = ActivityBounds;

    fn deref(&self) -> &Self::Target {
        &self.bounds
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

                            // // Get the highest ready to integrate sequence number for each agent.
                            // let activity_ready: Vec<(AgentPubKey, u32)> = txn
                            // .prepare_cached(
                            //     holochain_sqlite::sql::sql_cell::ACTIVITY_MISSING_DEP_UPPER_BOUND,
                            // )?
                            // .query_map(
                            //     named_params! {
                            //         ":register_activity": DhtOpType::RegisterAgentActivity,
                            //     },
                            //     |row| {
                            //         Ok((
                            //             row.get::<_, Option<AgentPubKey>>(0)?,
                            //             row.get::<_, Option<u32>>(1)?,
                            //         ))
                            //     },
                            // )?
                            // .filter_map(|r| match r {
                            //     Ok((a, seq)) => Some(Ok((a?, seq?))),
                            //     Err(e) => Some(Err(e)),
                            // })
                            // .collect::<rusqlite::Result<Vec<_>>>()?;

                            // dbg!(&activity_ready);

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

                    for ActivityState {
                        bounds,
                        out_of_order,
                    } in all_activity.values_mut()
                    {
                        // This vec is ordered in the query from lowest to highest header seq.
                        let last_consecutive_pos = out_of_order
                            .iter()
                            .zip(out_of_order.iter().skip(1))
                            .position(|(n, delta)| {
                                delta
                                    .checked_sub(1)
                                    .map(|delta_sub_1| delta_sub_1 != *n)
                                    .unwrap_or(true)
                            });
                        if let Some(pos) = last_consecutive_pos {
                            // Drop the consecutive seqs.
                            drop(out_of_order.drain(..=pos));
                            out_of_order.shrink_to_fit();
                            bounds.ready_to_integrate = Some(pos as u32);
                        }
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
    ) -> WorkflowResult<()> {
        self.get_or_try_init().await?.share_mut(|activity| {
            let mut new_bounds = ActivityBounds::default();
            for (author, seq_range) in integrated_activity {
                let prev_bounds = activity.get_mut(author.as_ref()).map(|s| &mut s.bounds);
                new_bounds.integrated = Some(*seq_range.start());
                if !update_activity_check(prev_bounds.as_deref(), &new_bounds) {
                    return Err(WorkflowError::ActivityOutOfOrder(
                        prev_bounds.and_then(|p| p.integrated).unwrap_or(0),
                        new_bounds.integrated.unwrap_or(0),
                    ));
                }
                new_bounds.integrated = Some(*seq_range.end());
                match prev_bounds {
                    Some(prev_bounds) => update_activity_inner(prev_bounds, &new_bounds)?,
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
    ) -> WorkflowResult<()> {
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
    ) -> WorkflowResult<()> {
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
    ) -> WorkflowResult<()> {
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
) -> WorkflowResult<()> {
    let prev_bounds = activity.get_mut(agent);
    if !update_activity_check(prev_bounds.as_deref(), new_bounds) {
        return Err(WorkflowError::ActivityOutOfOrder(
            prev_bounds.and_then(|p| p.integrated).unwrap_or(0),
            new_bounds.integrated.unwrap_or(0),
        ));
    }
    match prev_bounds {
        Some(prev_bounds) => update_activity_inner(prev_bounds, new_bounds)?,
        None => {
            activity.insert(Arc::new(agent.clone()), *new_bounds);
        }
    }
    WorkflowResult::Ok(())
}

fn update_activity_inner(
    prev_bounds: &mut ActivityBounds,
    new_bounds: &ActivityBounds,
) -> WorkflowResult<()> {
    if new_bounds.integrated.is_some() {
        prev_bounds.integrated = new_bounds.integrated;

        // If the "ready to integrate" sequence number was just
        // integrated then we can remove it.
        if prev_bounds
            .ready_to_integrate
            .and_then(|ready| prev_bounds.integrated.map(|integrated| integrated == ready))
            .unwrap_or(false)
        {
            prev_bounds.ready_to_integrate = None;
        }
    }

    // If there's already activity marked ready to integrate
    // we want to take the maximum of the two.
    if let Some(new_ready) = new_bounds.ready_to_integrate {
        prev_bounds.ready_to_integrate = Some(
            prev_bounds
                .ready_to_integrate
                .map_or(new_ready, |prev_ready| std::cmp::max(prev_ready, new_ready)),
        );
    }
    WorkflowResult::Ok(())
}
#[cfg(test)]
pub struct TestSpaces {
    pub spaces: Spaces,
    pub test_spaces: HashMap<DnaHash, TestSpace>,
    pub queue_consumer_map: QueueConsumerMap,
}
#[cfg(test)]
pub struct TestSpace {
    pub space: Space,
    _temp_dir: tempfile::TempDir,
}

impl Spaces {
    /// Create a new empty set of [`DnaHash`] spaces.
    pub fn new(
        root_env_dir: EnvironmentRootPath,
        db_sync_level: DbSyncStrategy,
        queue_consumer_map: QueueConsumerMap,
    ) -> Self {
        Spaces {
            map: RwShare::new(HashMap::new()),
            root_env_dir: Arc::new(root_env_dir),
            db_sync_level,
            queue_consumer_map,
        }
    }

    /// Get the space if it exists or create it if it doesn't.
    pub fn get_or_create_space(&self, dna_hash: &DnaHash) -> ConductorResult<Space> {
        self.get_or_create_space_ref(dna_hash, Space::clone)
    }

    fn get_or_create_space_ref<F, R>(&self, dna_hash: &DnaHash, mut f: F) -> ConductorResult<R>
    where
        F: FnMut(&Space) -> R,
    {
        match self
            .map
            .share_ref(|spaces| spaces.get(dna_hash).map(&mut f))
        {
            Some(r) => Ok(r),
            None => self
                .map
                .share_mut(|spaces| match spaces.entry(dna_hash.clone()) {
                    std::collections::hash_map::Entry::Occupied(entry) => Ok(f(entry.get())),
                    std::collections::hash_map::Entry::Vacant(entry) => {
                        let space = Space::new(
                            Arc::new(dna_hash.clone()),
                            &self.root_env_dir,
                            self.db_sync_level,
                        )?;

                        let r = f(&space);
                        entry.insert(space);
                        Ok(r)
                    }
                }),
        }
    }

    /// Get the cache database (this will create the space if it doesn't already exist).
    pub fn cache(&self, dna_hash: &DnaHash) -> ConductorResult<DbWrite<DbKindCache>> {
        self.get_or_create_space_ref(dna_hash, |space| space.cache.clone())
    }

    /// Get the authored database (this will create the space if it doesn't already exist).
    pub fn authored_env(&self, dna_hash: &DnaHash) -> ConductorResult<DbWrite<DbKindAuthored>> {
        self.get_or_create_space_ref(dna_hash, |space| space.authored_env.clone())
    }

    /// Get the dht database (this will create the space if it doesn't already exist).
    pub fn dht_env(&self, dna_hash: &DnaHash) -> ConductorResult<DbWrite<DbKindDht>> {
        self.get_or_create_space_ref(dna_hash, |space| space.dht_env.clone())
    }

    #[instrument(skip(self))]
    /// the network module is requesting a list of dht op hashes
    /// Get the [`DhtOpHash`]es and authored timestamps for a given time window.
    pub async fn handle_query_op_hashes(
        &self,
        dna_hash: &DnaHash,
        dht_arc_set: DhtArcSet,
        window: TimeWindow,
        max_ops: usize,
        include_limbo: bool,
    ) -> ConductorResult<Option<(Vec<DhtOpHash>, TimeWindowInclusive)>> {
        // The exclusive window bounds.
        let start = window.start;
        let end = window.end;
        let max_ops: u32 = max_ops.try_into().unwrap_or(u32::MAX);

        let env = self.dht_env(dna_hash)?;
        let include_limbo = include_limbo
            .then(|| "\n")
            .unwrap_or("AND DhtOp.when_integrated IS NOT NULL\n");

        let intervals = dht_arc_set.intervals();
        let sql = if let Some(ArcInterval::Full) = intervals.first() {
            format!(
                "{}{}{}",
                holochain_sqlite::sql::sql_cell::FETCH_OP_HASHES_P1,
                include_limbo,
                holochain_sqlite::sql::sql_cell::FETCH_OP_HASHES_P2,
            )
        } else {
            let sql_ranges = intervals
                .into_iter()
                .filter(|i| matches!(i, &ArcInterval::Bounded(_, _)))
                .map(|interval| match interval {
                    ArcInterval::Bounded(start_loc, end_loc) => {
                        if start_loc <= end_loc {
                            format!(
                                "AND storage_center_loc >= {} AND storage_center_loc <= {}",
                                start_loc, end_loc
                            )
                        } else {
                            format!(
                                "AND (storage_center_loc < {} OR storage_center_loc > {})",
                                end_loc, start_loc
                            )
                        }
                    }
                    _ => unreachable!(),
                })
                .collect::<String>();
            format!(
                "{}{}{}{}",
                holochain_sqlite::sql::sql_cell::FETCH_OP_HASHES_P1,
                include_limbo,
                sql_ranges,
                holochain_sqlite::sql::sql_cell::FETCH_OP_HASHES_P2,
            )
        };
        let results = env
            .async_reader(move |txn| {
                let hashes = txn
                    .prepare_cached(&sql)?
                    .query_map(
                        named_params! {
                            ":from": start,
                            ":to": end,
                            ":limit": max_ops,
                        },
                        |row| row.get("hash"),
                    )?
                    .collect::<rusqlite::Result<Vec<DhtOpHash>>>()?;
                let range = hashes.first().and_then(|s| hashes.last().map(|e| (s, e)));
                match range {
                    Some((start, end)) => {
                        let start: Timestamp = txn.query_row(
                            "SELECT authored_timestamp FROM DhtOp WHERE hash = ?",
                            [start],
                            |row| row.get(0),
                        )?;
                        let end: Timestamp = txn.query_row(
                            "SELECT authored_timestamp FROM DhtOp WHERE hash = ?",
                            [end],
                            |row| row.get(0),
                        )?;
                        DatabaseResult::Ok(Some((hashes, start..=end)))
                    }
                    None => Ok(None),
                }
            })
            .await?;

        Ok(results)
    }

    #[instrument(skip(self, op_hashes))]
    /// The network module is requesting the content for dht ops
    pub async fn handle_fetch_op_data(
        &self,
        dna_hash: &DnaHash,
        op_hashes: Vec<holo_hash::DhtOpHash>,
    ) -> ConductorResult<Vec<(holo_hash::DhtOpHash, holochain_types::dht_op::DhtOp)>> {
        const OPS_IN_MEMORY_BOUND_BYTES: usize = 3_000_000; // 3MB
                                                            // FIXME: Test this query.
        let env = self.dht_env(dna_hash)?;
        let results = env
            .async_reader(move |txn| {
                let mut out = Vec::with_capacity(op_hashes.len());
                let mut total_bytes = 0;
                for hash in op_hashes {
                    let r = txn.query_row_and_then(
                        "
                            SELECT DhtOp.hash, DhtOp.type AS dht_type,
                            Header.blob AS header_blob, Entry.blob AS entry_blob,
                            LENGTH(Header.blob) as header_size, LENGTH(Entry.blob) as entry_size
                            FROM DHtOp
                            JOIN Header ON DhtOp.header_hash = Header.hash
                            LEFT JOIN Entry ON Header.entry_hash = Entry.hash
                            WHERE
                            DhtOp.hash = ?
                            AND
                            DhtOp.when_integrated IS NOT NULL
                        ",
                        [hash],
                        |row| {
                            let header_bytes: Option<usize> = row.get("header_size")?;
                            let entry_bytes: Option<usize> = row.get("entry_size")?;
                            let bytes = header_bytes.unwrap_or(0) + entry_bytes.unwrap_or(0);
                            let header = from_blob::<SignedHeader>(row.get("header_blob")?)?;
                            let op_type: DhtOpType = row.get("dht_type")?;
                            let hash: DhtOpHash = row.get("hash")?;
                            // Check the entry isn't private before gossiping it.
                            let mut entry: Option<Entry> = None;
                            if header
                                .0
                                .entry_type()
                                .filter(|et| *et.visibility() == EntryVisibility::Public)
                                .is_some()
                            {
                                let e: Option<Vec<u8>> = row.get("entry_blob")?;
                                entry = match e {
                                    Some(entry) => Some(from_blob::<Entry>(entry)?),
                                    None => None,
                                };
                            }
                            let op = DhtOp::from_type(op_type, header, entry)?;
                            StateQueryResult::Ok(((hash, op), bytes))
                        },
                    );
                    match r {
                        Ok((r, bytes)) => {
                            out.push(r);
                            total_bytes += bytes;
                            if total_bytes > OPS_IN_MEMORY_BOUND_BYTES {
                                break;
                            }
                        }
                        Err(holochain_state::query::StateQueryError::Sql(
                            rusqlite::Error::QueryReturnedNoRows,
                        )) => (),
                        Err(e) => return Err(e),
                    }
                }
                StateQueryResult::Ok(out)
            })
            .await?;
        Ok(results)
    }

    #[instrument(skip(self, request_validation_receipt, ops))]
    /// we are receiving a "publish" event from the network
    pub async fn handle_publish(
        &self,
        dna_hash: &DnaHash,
        request_validation_receipt: bool,
        countersigning_session: bool,
        ops: Vec<(holo_hash::DhtOpHash, holochain_types::dht_op::DhtOp)>,
    ) -> ConductorResult<()> {
        // If this is a countersigning session then
        // send it to the countersigning workflow otherwise
        // send it to the incoming ops workflow.
        if countersigning_session {
            let (workspace, trigger) = self.get_or_create_space_ref(dna_hash, |space| {
                (
                    space.countersigning_workspace.clone(),
                    self.queue_consumer_map
                        .countersigning_trigger(space.dna_hash.clone()),
                )
            })?;
            let trigger = match trigger {
                Some(t) => t,
                // If the workflow has not been spawned yet we can't handle incoming messages.
                None => return Ok(()),
            };
            incoming_countersigning(ops, &workspace, trigger)?;
        } else {
            let space = self.get_or_create_space(dna_hash)?;
            let trigger = match self
                .queue_consumer_map
                .sys_validation_trigger(space.dna_hash.clone())
            {
                Some(t) => t,
                // If the workflow has not been spawned yet we can't handle incoming messages.
                // Not this is not an error because only a validation receipt is proof of a publish.
                None => return Ok(()),
            };
            incoming_dht_ops_workflow(&space, trigger, ops, request_validation_receipt).await?;
        }
        Ok(())
    }
}

impl Space {
    fn new(
        dna_hash: Arc<DnaHash>,
        root_env_dir: &EnvironmentRootPath,
        db_sync_strategy: DbSyncStrategy,
    ) -> ConductorResult<Self> {
        let cache = DbWrite::open_with_sync_level(
            root_env_dir.as_ref(),
            DbKindCache(dna_hash.clone()),
            match db_sync_strategy {
                DbSyncStrategy::Fast => DbSyncLevel::Off,
                DbSyncStrategy::Resilient => DbSyncLevel::Normal,
            },
        )?;
        let authored_env = DbWrite::open_with_sync_level(
            root_env_dir.as_ref(),
            DbKindAuthored(dna_hash.clone()),
            DbSyncLevel::Normal,
        )?;
        let dht_env = DbWrite::open_with_sync_level(
            root_env_dir.as_ref(),
            DbKindDht(dna_hash.clone()),
            match db_sync_strategy {
                DbSyncStrategy::Fast => DbSyncLevel::Off,
                DbSyncStrategy::Resilient => DbSyncLevel::Normal,
            },
        )?;
        let countersigning_workspace = CountersigningWorkspace::new();
        let incoming_op_hashes = IncomingOpHashes::default();
        let incoming_ops_batch = IncomingOpsBatch::default();
        let dht_query_cache = DhtDbQueryCache::new(dht_env.clone().into());
        let r = Self {
            dna_hash,
            cache,
            authored_env,
            dht_env,
            countersigning_workspace,
            incoming_op_hashes,
            incoming_ops_batch,
            dht_query_cache,
        };
        Ok(r)
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

#[cfg(test)]
impl TestSpaces {
    pub fn new(dna_hashes: impl IntoIterator<Item = DnaHash>) -> Self {
        let queue_consumer_map = QueueConsumerMap::new();
        Self::with_queue_consumer(dna_hashes, queue_consumer_map)
    }

    pub fn with_queue_consumer(
        dna_hashes: impl IntoIterator<Item = DnaHash>,
        queue_consumer_map: QueueConsumerMap,
    ) -> Self {
        let mut test_spaces: HashMap<DnaHash, _> = HashMap::new();
        for hash in dna_hashes.into_iter() {
            test_spaces.insert(hash.clone(), TestSpace::new(hash));
        }
        let temp_dir = tempfile::Builder::new()
            .prefix("holochain-test-environments")
            .tempdir()
            .unwrap();
        let spaces = Spaces::new(
            temp_dir.path().to_path_buf().into(),
            Default::default(),
            queue_consumer_map.clone(),
        );
        spaces.map.share_mut(|map| {
            map.extend(
                test_spaces
                    .iter()
                    .map(|(k, v)| (k.clone(), v.space.clone())),
            );
        });
        Self {
            queue_consumer_map,
            spaces,
            test_spaces,
        }
    }
}

#[cfg(test)]
impl TestSpace {
    pub fn new(dna_hash: DnaHash) -> Self {
        let temp_dir = tempfile::Builder::new()
            .prefix("holochain-test-environments")
            .tempdir()
            .unwrap();

        Self {
            space: Space::new(
                Arc::new(dna_hash),
                &temp_dir.path().to_path_buf().into(),
                Default::default(),
            )
            .unwrap(),
            _temp_dir: temp_dir,
        }
    }
}
