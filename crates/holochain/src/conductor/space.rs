//! This module contains data and functions for running operations
//! at the level of a [`DnaHash`] space.
//! Multiple [`Cell`](crate::conductor::Cell)'s could share the same space.
use std::{collections::HashMap, sync::Arc, time::Duration};

use super::{
    conductor::RwShare,
    error::ConductorResult,
    p2p_agent_store::{self, P2pBatch},
};
use crate::conductor::{error::ConductorError, state::ConductorState};
use crate::core::{
    queue_consumer::QueueConsumerMap,
    workflow::{
        countersigning_workflow::{incoming_countersigning, CountersigningWorkspace},
        incoming_dht_ops_workflow::{
            incoming_dht_ops_workflow, IncomingOpHashes, IncomingOpsBatch,
        },
    },
};
use holo_hash::{AgentPubKey, DhtOpHash, DnaHash};
use holochain_conductor_api::conductor::paths::DatabasesRootPath;
use holochain_conductor_api::conductor::ConductorConfig;
use holochain_keystore::MetaLairClient;
use holochain_p2p::AgentPubKeyExt;
use holochain_p2p::DnaHashExt;
use holochain_p2p::{
    dht::region::RegionBounds,
    dht_arc::{DhtArcRange, DhtArcSet},
    event::FetchOpDataQuery,
};
use holochain_sqlite::prelude::{
    DatabaseResult, DbKindAuthored, DbKindCache, DbKindConductor, DbKindDht, DbKindP2pAgents,
    DbKindP2pMetrics, DbKindWasm, DbSyncLevel, DbSyncStrategy, DbWrite, ReadAccess,
};
use holochain_state::{
    host_fn_workspace::SourceChainWorkspace,
    mutations,
    prelude::*,
    query::{map_sql_dht_op_common, StateQueryError},
};
use kitsune_p2p::event::{TimeWindow, TimeWindowInclusive};
use kitsune_p2p_block::NodeId;
use kitsune_p2p_types::agent_info::AgentInfoSigned;
use rusqlite::{named_params, OptionalExtension};
use std::convert::TryInto;
use std::path::PathBuf;
use tracing::instrument;

#[cfg(test)]
mod tests;

#[derive(Clone)]
/// This is the set of all current
/// [`DnaHash`] spaces for all cells
/// installed on this conductor.
pub struct Spaces {
    map: RwShare<HashMap<DnaHash, Space>>,
    pub(crate) db_dir: Arc<DatabasesRootPath>,
    pub(crate) config: Arc<ConductorConfig>,
    /// The map of running queue consumer workflows.
    pub(crate) queue_consumer_map: QueueConsumerMap,
    pub(crate) conductor_db: DbWrite<DbKindConductor>,
    pub(crate) wasm_db: DbWrite<DbKindWasm>,
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
    pub cache_db: DbWrite<DbKindCache>,

    /// The conductor database. There is only one of these.
    pub conductor_db: DbWrite<DbKindConductor>,

    /// The authored databases. These are shared across cells.
    /// There is one per unique Dna.
    pub authored_db: DbWrite<DbKindAuthored>,

    /// The dht databases. These are shared across cells.
    /// There is one per unique Dna.
    pub dht_db: DbWrite<DbKindDht>,

    /// The database for storing AgentInfoSigned
    pub p2p_agents_db: DbWrite<DbKindP2pAgents>,

    /// The database for storing p2p MetricDatum(s)
    pub p2p_metrics_db: DbWrite<DbKindP2pMetrics>,

    /// The batch sender for writes to the p2p database.
    pub p2p_batch_sender: tokio::sync::mpsc::Sender<P2pBatch>,

    /// A cache for slow database queries.
    pub dht_query_cache: DhtDbQueryCache,

    /// Countersigning workspace that is shared across this cell.
    pub countersigning_workspace: CountersigningWorkspace,

    /// Incoming op hashes that are queued for processing.
    pub incoming_op_hashes: IncomingOpHashes,

    /// Incoming ops batch for this space.
    pub incoming_ops_batch: IncomingOpsBatch,
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
    pub fn new(config: Arc<ConductorConfig>) -> ConductorResult<Self> {
        let root_db_dir: DatabasesRootPath = config
            .data_root_path
            .clone()
            .ok_or(ConductorError::NoDataRootPath)?
            .try_into()?;
        let db_sync_strategy = config.db_sync_strategy;
        let db_sync_level = match db_sync_strategy {
            DbSyncStrategy::Fast => DbSyncLevel::Off,
            DbSyncStrategy::Resilient => DbSyncLevel::Normal,
        };
        let conductor_db =
            DbWrite::open_with_sync_level(root_db_dir.as_ref(), DbKindConductor, db_sync_level)?;
        let wasm_db =
            DbWrite::open_with_sync_level(root_db_dir.as_ref(), DbKindWasm, db_sync_level)?;
        Ok(Spaces {
            map: RwShare::new(HashMap::new()),
            db_dir: Arc::new(root_db_dir),
            config,
            queue_consumer_map: QueueConsumerMap::new(),
            conductor_db,
            wasm_db,
        })
    }

    /// Block some target.
    pub async fn block(&self, input: Block) -> DatabaseResult<()> {
        holochain_state::block::block(&self.conductor_db, input).await
    }

    /// Unblock some target.
    pub async fn unblock(&self, input: Block) -> DatabaseResult<()> {
        holochain_state::block::unblock(&self.conductor_db, input).await
    }

    async fn node_agents_in_spaces(
        &self,
        node_id: NodeId,
        dnas: Vec<DnaHash>,
    ) -> DatabaseResult<Vec<CellId>> {
        let mut agent_lists: Vec<Vec<AgentInfoSigned>> = vec![];
        for dna in dnas {
            // @todo join_all for these awaits
            agent_lists.push(self.p2p_agents_db(&dna)?.p2p_list_agents().await?);
        }

        Ok(agent_lists
            .into_iter()
            .flatten()
            .filter(|agent| {
                agent.url_list.iter().any(|url| {
                    kitsune_p2p::dependencies::kitsune_p2p_proxy::ProxyUrl::from(url.as_str())
                        .digest()
                        .0
                        == *node_id
                })
            })
            .map(|agent_info| {
                CellId::new(
                    DnaHash::from_kitsune(&agent_info.space),
                    AgentPubKey::from_kitsune(&agent_info.agent),
                )
            })
            .collect())
    }

    /// Check if some target is blocked.
    pub async fn is_blocked(
        &self,
        target_id: BlockTargetId,
        timestamp: Timestamp,
    ) -> DatabaseResult<bool> {
        let cell_ids = match &target_id {
            BlockTargetId::Cell(cell_id) => vec![cell_id.to_owned()],
            BlockTargetId::NodeDna(node_id, dna_hash) => {
                self.node_agents_in_spaces((*node_id).clone(), vec![dna_hash.clone()])
                    .await?
            }
            BlockTargetId::Node(node_id) => {
                self.node_agents_in_spaces(
                    (*node_id).clone(),
                    self.map
                        .share_ref(|m| m.keys().cloned().collect::<Vec<DnaHash>>()),
                )
                .await?
            }
            // @todo
            BlockTargetId::Ip(_) => {
                vec![]
            }
        };

        // If node_agents_in_spaces is not yet initialized, we can't know anything about
        // which cells are blocked, so avoid the race condition by returning false
        // TODO: actually fix the preflight, because this could be a loophole for someone
        //       to evade a block in some circumstances
        if cell_ids.is_empty() {
            return Ok(false);
        }

        self.conductor_db
            .read_async(move |txn| {
                Ok(
                    // If the target_id is directly blocked then we always return true.
                    holochain_state::block::query_is_blocked(&txn, target_id, timestamp)?
            // If there are zero unblocked cells then return true.
            || {
                let mut all_blocked_cell_ids = true;
                for cell_id in cell_ids {
                    if !holochain_state::block::query_is_blocked(
                        &txn,
                        BlockTargetId::Cell(cell_id), timestamp)? {
                            all_blocked_cell_ids = false;
                            break;
                        }
                }
                all_blocked_cell_ids
            },
                )
            })
            .await
    }

    /// Get the holochain conductor state
    pub async fn get_state(&self) -> ConductorResult<ConductorState> {
        match query_conductor_state(&self.conductor_db).await? {
            Some(state) => Ok(state),
            // update_state will again try to read the state. It's a little
            // inefficient in the infrequent case where we haven't saved the
            // state yet, but more atomic, so worth it.
            None => self.update_state(Ok).await,
        }
    }

    /// Update the internal state with a pure function mapping old state to new
    pub async fn update_state<F: Send>(&self, f: F) -> ConductorResult<ConductorState>
    where
        F: FnOnce(ConductorState) -> ConductorResult<ConductorState> + 'static,
    {
        let (state, _) = self.update_state_prime(|s| Ok((f(s)?, ()))).await?;
        Ok(state)
    }

    /// Update the internal state with a pure function mapping old state to new,
    /// which may also produce an output value which will be the output of
    /// this function
    pub async fn update_state_prime<F, O>(&self, f: F) -> ConductorResult<(ConductorState, O)>
    where
        F: FnOnce(ConductorState) -> ConductorResult<(ConductorState, O)> + Send + 'static,
        O: Send + 'static,
    {
        let output = self
            .conductor_db
            .write_async(move |txn| {
                let state = txn
                    .query_row("SELECT blob FROM ConductorState WHERE id = 1", [], |row| {
                        row.get("blob")
                    })
                    .optional()?;
                let state = match state {
                    Some(state) => from_blob(state)?,
                    None => ConductorState::default(),
                };
                let (new_state, output) = f(state)?;
                mutations::insert_conductor_state(txn, (&new_state).try_into()?)?;
                Result::<_, ConductorError>::Ok((new_state, output))
            })
            .await?;
        Ok(output)
    }

    /// Get something from every space
    pub fn get_from_spaces<R, F: FnMut(&Space) -> R>(&self, f: F) -> Vec<R> {
        self.map
            .share_ref(|spaces| spaces.values().map(f).collect())
    }

    /// Get the space if it exists or create it if it doesn't.
    pub fn get_or_create_space(&self, dna_hash: &DnaHash) -> DatabaseResult<Space> {
        self.get_or_create_space_ref(dna_hash, Space::clone)
    }

    fn get_or_create_space_ref<F, R>(&self, dna_hash: &DnaHash, f: F) -> DatabaseResult<R>
    where
        F: Fn(&Space) -> R,
    {
        match self.map.share_ref(|spaces| spaces.get(dna_hash).map(&f)) {
            Some(r) => Ok(r),
            None => self
                .map
                .share_mut(|spaces| match spaces.entry(dna_hash.clone()) {
                    std::collections::hash_map::Entry::Occupied(entry) => Ok(f(entry.get())),
                    std::collections::hash_map::Entry::Vacant(entry) => {
                        let space = Space::new(
                            Arc::new(dna_hash.clone()),
                            &self.db_dir,
                            self.config.db_sync_strategy,
                        )?;

                        let r = f(&space);
                        entry.insert(space);
                        Ok(r)
                    }
                }),
        }
    }

    /// Get the cache database (this will create the space if it doesn't already exist).
    pub fn cache(&self, dna_hash: &DnaHash) -> DatabaseResult<DbWrite<DbKindCache>> {
        self.get_or_create_space_ref(dna_hash, |space| space.cache_db.clone())
    }

    /// Get the authored database (this will create the space if it doesn't already exist).
    pub fn authored_db(&self, dna_hash: &DnaHash) -> DatabaseResult<DbWrite<DbKindAuthored>> {
        self.get_or_create_space_ref(dna_hash, |space| space.authored_db.clone())
    }

    /// Get the dht database (this will create the space if it doesn't already exist).
    pub fn dht_db(&self, dna_hash: &DnaHash) -> DatabaseResult<DbWrite<DbKindDht>> {
        self.get_or_create_space_ref(dna_hash, |space| space.dht_db.clone())
    }

    /// Get the peer database (this will create the space if it doesn't already exist).
    pub fn p2p_agents_db(&self, dna_hash: &DnaHash) -> DatabaseResult<DbWrite<DbKindP2pAgents>> {
        self.get_or_create_space_ref(dna_hash, |space| space.p2p_agents_db.clone())
    }

    /// Get the peer database (this will create the space if it doesn't already exist).
    pub fn p2p_metrics_db(&self, dna_hash: &DnaHash) -> DatabaseResult<DbWrite<DbKindP2pMetrics>> {
        self.get_or_create_space_ref(dna_hash, |space| space.p2p_metrics_db.clone())
    }

    /// Get the batch sender (this will create the space if it doesn't already exist).
    pub fn p2p_batch_sender(
        &self,
        dna_hash: &DnaHash,
    ) -> DatabaseResult<tokio::sync::mpsc::Sender<P2pBatch>> {
        self.get_or_create_space_ref(dna_hash, |space| space.p2p_batch_sender.clone())
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

        let db = self.dht_db(dna_hash)?;

        let include_limbo = if include_limbo {
            "\n"
        } else {
            "AND DhtOp.when_integrated IS NOT NULL\n"
        };

        let intervals = dht_arc_set.intervals();
        let sql = if let Some(DhtArcRange::Full) = intervals.first() {
            format!(
                "{} {} {}",
                holochain_sqlite::sql::sql_cell::FETCH_OP_HASHES_P1,
                include_limbo,
                holochain_sqlite::sql::sql_cell::FETCH_OP_HASHES_P2,
            )
        } else {
            let sql_ranges = intervals
                .into_iter()
                .filter(|i| matches!(i, &DhtArcRange::Bounded(_, _)))
                .map(|interval| match interval {
                    DhtArcRange::Bounded(start_loc, end_loc) => {
                        if start_loc <= end_loc {
                            format!(
                                "AND storage_center_loc >= {} AND storage_center_loc <= {} \n ",
                                start_loc, end_loc
                            )
                        } else {
                            format!(
                                "AND (storage_center_loc < {} OR storage_center_loc > {}) \n ",
                                end_loc, start_loc
                            )
                        }
                    }
                    _ => unreachable!(),
                })
                .collect::<String>();
            format!(
                "{} {} {} {}",
                holochain_sqlite::sql::sql_cell::FETCH_OP_HASHES_P1,
                include_limbo,
                sql_ranges,
                holochain_sqlite::sql::sql_cell::FETCH_OP_HASHES_P2,
            )
        };
        let results = db
            .read_async(move |txn| {
                let mut stmt = txn.prepare_cached(&sql)?;
                let hashes = stmt
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

    #[instrument(skip(self, query))]
    /// The network module is requesting the content for dht ops
    pub async fn handle_fetch_op_data(
        &self,
        dna_hash: &DnaHash,
        query: FetchOpDataQuery,
    ) -> ConductorResult<Vec<(holo_hash::DhtOpHash, holochain_types::dht_op::DhtOp)>> {
        match query {
            FetchOpDataQuery::Hashes {
                op_hash_list,
                include_limbo,
            } => {
                self.handle_fetch_op_data_by_hashes(dna_hash, op_hash_list, include_limbo)
                    .await
            }
            FetchOpDataQuery::Regions(regions) => {
                self.handle_fetch_op_data_by_regions(dna_hash, regions)
                    .await
            }
        }
    }

    #[instrument(skip(self, regions))]
    /// The network module is requesting the content for dht ops
    pub async fn handle_fetch_op_data_by_regions(
        &self,
        dna_hash: &DnaHash,
        regions: Vec<RegionBounds>,
    ) -> ConductorResult<Vec<(holo_hash::DhtOpHash, holochain_types::dht_op::DhtOp)>> {
        let sql = holochain_sqlite::sql::sql_cell::FETCH_OPS_BY_REGION;
        Ok(self
            .dht_db(dna_hash)?
            .read_async(move |txn| {
                let mut stmt = txn.prepare_cached(sql).map_err(StateQueryError::from)?;
                let results = regions
                    .into_iter()
                    .map(|bounds| {
                        let (x0, x1) = bounds.x;
                        let (t0, t1) = bounds.t;
                        stmt.query_and_then(
                            named_params! {
                                ":storage_start_loc": x0,
                                ":storage_end_loc": x1,
                                ":timestamp_min": t0,
                                ":timestamp_max": t1,
                            },
                            |row| {
                                let hash: DhtOpHash =
                                    row.get("hash").map_err(StateQueryError::from)?;
                                Ok(map_sql_dht_op_common(row)?.map(|op| (hash, op)))
                            },
                        )
                        .map_err(StateQueryError::from)?
                        .collect::<Result<Vec<Option<_>>, StateQueryError>>()
                    })
                    .collect::<Result<Vec<Vec<Option<_>>>, _>>()?
                    .into_iter()
                    .flatten()
                    .flatten()
                    .collect();
                StateQueryResult::Ok(results)
            })
            .await?)
    }

    #[instrument(skip(self, op_hashes))]
    /// The network module is requesting the content for dht ops
    pub async fn handle_fetch_op_data_by_hashes(
        &self,
        dna_hash: &DnaHash,
        op_hashes: Vec<holo_hash::DhtOpHash>,
        include_limbo: bool,
    ) -> ConductorResult<Vec<(holo_hash::DhtOpHash, holochain_types::dht_op::DhtOp)>> {
        let mut sql = "
            SELECT DhtOp.hash, DhtOp.type AS dht_type,
            Action.blob AS action_blob, Entry.blob AS entry_blob
            FROM DHtOp
            JOIN Action ON DhtOp.action_hash = Action.hash
            LEFT JOIN Entry ON Action.entry_hash = Entry.hash
            WHERE
            DhtOp.hash = ?
        "
        .to_string();

        if !include_limbo {
            sql.push_str(
                "
                AND
                DhtOp.when_integrated IS NOT NULL
            ",
            );
        }

        let db = self.dht_db(dna_hash)?;
        let results = db
            .read_async(move |txn| {
                let mut out = Vec::with_capacity(op_hashes.len());
                for hash in op_hashes {
                    let mut stmt = txn.prepare_cached(&sql)?;
                    let mut rows = stmt.query([hash])?;
                    if let Some(row) = rows.next()? {
                        let action = from_blob::<SignedAction>(row.get("action_blob")?)?;
                        let op_type: DhtOpType = row.get("dht_type")?;
                        let hash: DhtOpHash = row.get("hash")?;
                        // Check the entry isn't private before gossiping it.
                        let mut entry: Option<Entry> = None;
                        if action
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
                        let op = DhtOp::from_type(op_type, action, entry)?;
                        out.push((hash, op))
                    } else {
                        return Err(holochain_state::query::StateQueryError::Sql(
                            rusqlite::Error::QueryReturnedNoRows,
                        ));
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
        ops: Vec<DhtOp>,
    ) -> ConductorResult<()> {
        // If this is a countersigning session then
        // send it to the countersigning workflow otherwise
        // send it to the incoming ops workflow.
        if countersigning_session {
            use futures::StreamExt;
            let ops = futures::stream::iter(ops.into_iter().map(|op| {
                let hash = DhtOpHash::with_data_sync(&op);
                (hash, op)
            }))
            .collect()
            .await;

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
                // Note this is not an error because only a validation receipt is proof of a publish.
                None => {
                    tracing::warn!("No sys validation trigger yet for space: {}", dna_hash);
                    return Ok(());
                }
            };
            incoming_dht_ops_workflow(space, trigger, ops, request_validation_receipt).await?;
        }
        Ok(())
    }

    /// Get the recent_threshold based on the kitsune network config
    pub fn recent_threshold(&self) -> Duration {
        self.config
            .network
            .tuning_params
            .danger_gossip_recent_threshold()
    }
}

impl Space {
    fn new(
        dna_hash: Arc<DnaHash>,
        root_db_dir: &PathBuf,
        db_sync_strategy: DbSyncStrategy,
    ) -> DatabaseResult<Self> {
        let space = dna_hash.to_kitsune();
        let db_sync_level = match db_sync_strategy {
            DbSyncStrategy::Fast => DbSyncLevel::Off,
            DbSyncStrategy::Resilient => DbSyncLevel::Normal,
        };
        let cache = DbWrite::open_with_sync_level(
            root_db_dir.as_ref(),
            DbKindCache(dna_hash.clone()),
            db_sync_level,
        )?;
        let authored_db = DbWrite::open_with_sync_level(
            root_db_dir.as_ref(),
            DbKindAuthored(dna_hash.clone()),
            DbSyncLevel::Normal,
        )?;
        let dht_db = DbWrite::open_with_sync_level(
            root_db_dir.as_ref(),
            DbKindDht(dna_hash.clone()),
            db_sync_level,
        )?;
        let p2p_agents_db = DbWrite::open_with_sync_level(
            root_db_dir.as_ref(),
            DbKindP2pAgents(space.clone()),
            db_sync_level,
        )?;
        let p2p_metrics_db = DbWrite::open_with_sync_level(
            root_db_dir.as_ref(),
            DbKindP2pMetrics(space),
            db_sync_level,
        )?;
        let conductor_db: DbWrite<DbKindConductor> =
            DbWrite::open_with_sync_level(root_db_dir.as_ref(), DbKindConductor, db_sync_level)?;

        let (tx, rx) = tokio::sync::mpsc::channel(100);
        tokio::spawn(p2p_agent_store::p2p_put_all_batch(
            p2p_agents_db.clone(),
            rx,
        ));
        let p2p_batch_sender = tx;

        let countersigning_workspace = CountersigningWorkspace::new();
        let incoming_op_hashes = IncomingOpHashes::default();
        let incoming_ops_batch = IncomingOpsBatch::default();
        let dht_query_cache = DhtDbQueryCache::new(dht_db.clone().into());
        let r = Self {
            dna_hash,
            cache_db: cache,
            authored_db,
            dht_db,
            p2p_agents_db,
            p2p_metrics_db,
            p2p_batch_sender,
            countersigning_workspace,
            incoming_op_hashes,
            incoming_ops_batch,
            dht_query_cache,
            conductor_db,
        };
        Ok(r)
    }

    /// Construct a SourceChain for an author in this Space
    pub async fn source_chain(
        &self,
        keystore: MetaLairClient,
        author: AgentPubKey,
    ) -> SourceChainResult<SourceChain> {
        SourceChain::raw_empty(
            self.authored_db.clone(),
            self.dht_db.clone(),
            self.dht_query_cache.clone(),
            keystore,
            author,
        )
        .await
    }

    /// Create a SourceChainWorkspace from this Space
    pub async fn source_chain_workspace(
        &self,
        keystore: MetaLairClient,
        agent_pubkey: AgentPubKey,
        dna_def: Arc<DnaDef>,
    ) -> ConductorResult<SourceChainWorkspace> {
        Ok(SourceChainWorkspace::new(
            self.authored_db.clone(),
            self.dht_db.clone(),
            self.dht_query_cache.clone(),
            self.cache_db.clone(),
            keystore,
            agent_pubkey,
            dna_def,
        )
        .await?)
    }
}

/// Get the holochain conductor state
pub async fn query_conductor_state(
    db: &DbRead<DbKindConductor>,
) -> ConductorResult<Option<ConductorState>> {
    db.read_async(|txn| {
        let state = txn
            .query_row("SELECT blob FROM ConductorState WHERE id = 1", [], |row| {
                row.get("blob")
            })
            .optional()?;
        match state {
            Some(state) => ConductorResult::Ok(Some(from_blob(state)?)),
            None => ConductorResult::Ok(None),
        }
    })
    .await
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
            ConductorConfig {
                data_root_path: Some(temp_dir.path().to_path_buf().into()),
                ..Default::default()
            }
            .into(),
        )
        .unwrap();
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
