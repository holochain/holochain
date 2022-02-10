//! This module contains data and functions for running operations
//! at the level of a [`DnaHash`] space.
//! Multiple [`Cell`]'s could share the same space.

use std::{collections::HashMap, sync::Arc};

use holo_hash::{DhtOpHash, DnaHash};
use holochain_conductor_api::conductor::EnvironmentRootPath;
use holochain_p2p::{
    dht::region::{RegionCoordSetXtcs, RegionData},
    dht_arc::{ArcInterval, DhtArcSet},
};
use holochain_sqlite::{
    conn::{DbSyncLevel, DbSyncStrategy},
    db::{DbKindAuthored, DbKindCache, DbKindDht, DbWrite},
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
        incoming_dht_ops_workflow::{
            incoming_dht_ops_workflow, IncomingOpHashes, IncomingOpsBatch,
        },
    },
};

use super::{conductor::RwShare, error::ConductorResult};
use std::convert::TryInto;

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
    _temp_dir: tempdir::TempDir,
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

    /// The network module needs info about various groupings ("regions") of ops
    pub async fn handle_fetch_op_regions(
        &self,
        dna_hash: &DnaHash,
        regions: RegionCoordSetXtcs,
    ) -> ConductorResult<Vec<Vec<RegionData>>> {
        todo!("DB query to build region data")
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
        let r = Self {
            dna_hash,
            cache,
            authored_env,
            dht_env,
            countersigning_workspace,
            incoming_op_hashes,
            incoming_ops_batch,
        };
        Ok(r)
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
        use tempdir::TempDir;
        let mut test_spaces: HashMap<DnaHash, _> = HashMap::new();
        for hash in dna_hashes.into_iter() {
            test_spaces.insert(hash.clone(), TestSpace::new(hash));
        }
        let temp_dir = TempDir::new("holochain-test-environments").unwrap();
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
        use tempdir::TempDir;

        let temp_dir = TempDir::new("holochain-test-environments").unwrap();

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
