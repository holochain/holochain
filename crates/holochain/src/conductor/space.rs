//! This module contains data and functions for running operations
//! at the level of a [`DnaHash`] space.
//! Multiple [`Cell`](crate::conductor::Cell)'s could share the same space.
use super::{conductor::RwShare, error::ConductorResult};
use crate::conductor::{error::ConductorError, state::ConductorState};
use crate::core::workflow::countersigning_workflow::CountersigningWorkspace;
use crate::core::{
    queue_consumer::QueueConsumerMap,
    workflow::{
        incoming_dht_ops_workflow::{
            incoming_dht_ops_workflow, IncomingOpHashes, IncomingOpsBatch,
        },
        witnessing_workflow::{receive_incoming_countersigning_ops, WitnessingWorkspace},
    },
};
use holo_hash::{AgentPubKey, DhtOpHash, DnaHash};
use holochain_conductor_api::conductor::paths::DatabasesRootPath;
use holochain_conductor_api::conductor::ConductorConfig;
use holochain_keystore::MetaLairClient;
use holochain_p2p::actor::DynHcP2p;
use holochain_sqlite::prelude::{
    DatabaseResult, DbKey, DbKindAuthored, DbKindCache, DbKindConductor, DbKindDht, DbSyncLevel,
    DbSyncStrategy, DbWrite, PoolConfig, ReadAccess,
};
use holochain_state::{host_fn_workspace::SourceChainWorkspace, mutations, prelude::*};
use holochain_util::timed;
use lair_keystore_api::prelude::SharedLockedArray;
use rusqlite::OptionalExtension;
use std::convert::TryInto;
use std::path::PathBuf;
use std::{
    cell::Cell,
    collections::{hash_map, HashMap},
    sync::Arc,
};

/// This is the set of all current
/// [`DnaHash`] spaces for all cells
/// installed on this conductor.
#[derive(Clone)]
pub struct Spaces {
    map: RwShare<HashMap<DnaHash, Space>>,
    pub(crate) db_dir: Arc<DatabasesRootPath>,
    pub(crate) config: Arc<ConductorConfig>,
    /// The map of running queue consumer workflows.
    pub(crate) queue_consumer_map: QueueConsumerMap,
    pub(crate) conductor_db: DbWrite<DbKindConductor>,
    pub(crate) wasm_store: holochain_state::wasm::WasmStore,
    pub(crate) dna_def_store: holochain_state::dna_def::DnaDefStore,
    pub(crate) entry_def_store: holochain_state::entry_def::EntryDefStore,
    db_key: DbKey,
}

/// This is the set of data required at the
/// [`DnaHash`] space level.
/// All cells in the same [`DnaHash`] space
/// will share these.
#[derive(Clone)]
pub struct Space {
    /// The dna hash for this space.
    pub dna_hash: Arc<DnaHash>,

    /// The caches databases. These are shared across cells.
    /// There is one per unique Dna.
    pub cache_db: DbWrite<DbKindCache>,

    /// The conductor database. There is only one of these.
    pub conductor_db: DbWrite<DbKindConductor>,

    /// The authored databases. These are per-agent.
    /// There is one per unique combination of Dna and AgentPubKey.
    pub authored_dbs: Arc<parking_lot::Mutex<HashMap<AgentPubKey, DbWrite<DbKindAuthored>>>>,

    /// The dht databases. These are shared across cells.
    /// There is one per unique Dna.
    pub dht_db: DbWrite<DbKindDht>,

    /// The peer meta store database. One per unique Dna.
    pub peer_meta_store_db: DbWrite<DbKindPeerMetaStore>,

    /// Countersigning workspace for session state.
    pub countersigning_workspaces:
        Arc<parking_lot::Mutex<HashMap<CellId, Arc<CountersigningWorkspace>>>>,

    /// Witnessing workspace that is shared across this cell.
    pub witnessing_workspace: WitnessingWorkspace,

    /// Incoming op hashes that are queued for processing.
    pub incoming_op_hashes: IncomingOpHashes,

    /// Incoming ops batch for this space.
    pub incoming_ops_batch: IncomingOpsBatch,

    root_db_dir: Arc<PathBuf>,
    db_key: DbKey,
    db_max_readers: u16,
}

/// Test spaces
#[cfg(test)]
pub struct TestSpaces {
    /// The spaces
    pub spaces: Spaces,
    /// The test spaces
    pub test_spaces: HashMap<DnaHash, TestSpace>,
    /// The queue consumer map
    pub queue_consumer_map: QueueConsumerMap,
}

/// A test space
#[cfg(test)]
pub struct TestSpace {
    /// The space
    pub space: Space,
    _temp_dir: tempfile::TempDir,
}

thread_local!(static DANGER_PRINT_DB_SECRETS: Cell<bool> = const { Cell::new(false) });

/// WARNING!! DANGER!! This exposes your database decryption secrets!
/// Print the database decryption secrets to stderr.
/// With these PRAGMA commands, you'll be able to run sqlcipher
/// directly to manipulate holochain databases.
pub fn set_danger_print_db_secrets(v: bool) {
    DANGER_PRINT_DB_SECRETS.set(v);
}

impl Spaces {
    /// Create a new empty set of [`DnaHash`] spaces.
    pub async fn new(
        config: Arc<ConductorConfig>,
        passphrase: SharedLockedArray,
    ) -> ConductorResult<Self> {
        // do this before any awaits
        let danger_print_db_secrets = DANGER_PRINT_DB_SECRETS.get();
        // clear the value
        DANGER_PRINT_DB_SECRETS.set(false);

        let root_db_dir: DatabasesRootPath = config
            .data_root_path
            .clone()
            .ok_or(ConductorError::NoDataRootPath)?
            .try_into()?;

        let db_key_path = root_db_dir.join("db.key");
        let db_key = match tokio::fs::read_to_string(db_key_path.clone()).await {
            Ok(locked) => DbKey::load(locked, passphrase.clone()).await?,
            Err(_) => {
                let db_key = DbKey::generate(passphrase.clone()).await?;
                tokio::fs::write(db_key_path, db_key.locked.clone()).await?;
                db_key
            }
        };

        if danger_print_db_secrets {
            eprintln!(
                "--beg-db-secrets--{}--end-db-secrets--",
                &String::from_utf8_lossy(&*db_key.unlocked.lock().unwrap().lock())
            );
        }

        let db_sync_strategy = config.db_sync_strategy;
        let db_sync_level = match db_sync_strategy {
            DbSyncStrategy::Fast => DbSyncLevel::Off,
            DbSyncStrategy::Resilient => DbSyncLevel::Normal,
        };

        let conductor_db = tokio::task::block_in_place(|| {
            let conductor_db = DbWrite::open_with_pool_config(
                root_db_dir.as_ref(),
                DbKindConductor,
                PoolConfig {
                    synchronous_level: db_sync_level,
                    key: db_key.clone(),
                    max_readers: config.db_max_readers,
                },
            )?;
            ConductorResult::Ok(conductor_db)
        })?;

        let db_sync = match db_sync_level {
            DbSyncLevel::Off => holochain_data::DbSyncLevel::Off,
            DbSyncLevel::Normal => holochain_data::DbSyncLevel::Normal,
            DbSyncLevel::Full => holochain_data::DbSyncLevel::Full,
        };

        // Convert the DbKey from holochain_sqlite to holochain_data format
        let data_db_key = holochain_data::DbKey::load(db_key.locked.clone(), passphrase.clone())
            .await
            .map_err(ConductorError::other)?;

        let wasm_db = holochain_data::open_db(
            root_db_dir.as_ref(),
            holochain_data::kind::Wasm,
            holochain_data::HolochainDataConfig {
                key: Some(data_db_key),
                sync_level: db_sync,
                max_readers: config.db_max_readers,
            },
        )
        .await
        .map_err(|e| std::io::Error::other(e.to_string()))?;

        // Create store instances from the wasm database
        let wasm_store = holochain_state::wasm::WasmStore::new(wasm_db.clone());
        let dna_def_store = holochain_state::dna_def::DnaDefStore::new(wasm_db.clone());
        let entry_def_store = holochain_state::entry_def::EntryDefStore::new(wasm_db);

        Ok(Spaces {
            map: RwShare::new(HashMap::new()),
            db_dir: Arc::new(root_db_dir),
            config,
            queue_consumer_map: QueueConsumerMap::new(),
            conductor_db,
            wasm_store,
            dna_def_store,
            entry_def_store,
            db_key,
        })
    }

    /// Unblock some target.
    pub async fn unblock(&self, input: Block) -> DatabaseResult<()> {
        holochain_state::block::unblock(&self.conductor_db, input).await
    }

    /// Check if some target is blocked.
    pub async fn is_blocked(
        &self,
        target_id: BlockTargetId,
        timestamp: Timestamp,
        _holochain_p2p: DynHcP2p,
    ) -> ConductorResult<bool> {
        let cell_ids = match &target_id {
            BlockTargetId::Cell(cell_id) => vec![cell_id.to_owned()],
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
                    holochain_state::block::query_is_blocked(txn, target_id, timestamp)?
            // If there are zero unblocked cells then return true.
            || {
                let mut all_blocked_cell_ids = true;
                for cell_id in cell_ids {
                    if !holochain_state::block::query_is_blocked(
                        txn,
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
    #[cfg_attr(feature = "instrument", tracing::instrument(skip_all))]
    pub async fn get_state(&self) -> ConductorResult<ConductorState> {
        timed!([1, 10, 1000], "get_state", {
            match query_conductor_state(&self.conductor_db).await? {
                Some(state) => Ok(state),
                // update_state will again try to read the state. It's a little
                // inefficient in the infrequent case where we haven't saved the
                // state yet, but more atomic, so worth it.
                None => self.update_state(Ok).await,
            }
        })
    }

    /// Update the internal state with a pure function mapping old state to new
    #[cfg_attr(feature = "instrument", tracing::instrument(skip_all))]
    pub async fn update_state<F>(&self, f: F) -> ConductorResult<ConductorState>
    where
        F: Send + FnOnce(ConductorState) -> ConductorResult<ConductorState> + 'static,
    {
        let (state, _) = self.update_state_prime(|s| Ok((f(s)?, ()))).await?;
        Ok(state)
    }

    /// Update the internal state with a pure function mapping old state to new,
    /// which may also produce an output value which will be the output of
    /// this function
    #[cfg_attr(feature = "instrument", tracing::instrument(skip_all))]
    pub async fn update_state_prime<F, O>(&self, f: F) -> ConductorResult<(ConductorState, O)>
    where
        F: FnOnce(ConductorState) -> ConductorResult<(ConductorState, O)> + Send + 'static,
        O: Send + 'static,
    {
        timed!([1, 10, 1000], "update_state_prime", {
            self.conductor_db
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
                .await
        })
    }

    /// Get something from every space
    pub fn get_from_spaces<R, F: FnMut(&Space) -> R>(&self, f: F) -> Vec<R> {
        self.map
            .share_ref(|spaces| spaces.values().map(f).collect())
    }

    /// Get the space if it exists or create it if it doesn't.
    pub fn get_or_create_space(&self, dna_hash: &DnaHash) -> DatabaseResult<Space> {
        self.get_or_create_space_ref(dna_hash, |s| s.clone())
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
                    hash_map::Entry::Occupied(entry) => Ok(f(entry.get())),
                    hash_map::Entry::Vacant(entry) => {
                        let space = Space::new(
                            Arc::new(dna_hash.clone()),
                            self.db_dir.to_path_buf(),
                            self.config.db_sync_strategy,
                            self.db_key.clone(),
                            self.config.db_max_readers,
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

    /// Get or create the authored database for this author (this will create the space if it doesn't already exist).
    pub fn get_or_create_authored_db(
        &self,
        dna_hash: &DnaHash,
        author: AgentPubKey,
    ) -> DatabaseResult<DbWrite<DbKindAuthored>> {
        self.get_or_create_space_ref(dna_hash, |space| {
            space.get_or_create_authored_db(author.clone())
        })?
    }

    /// Get all the authored databases for this space (this will create the space if it doesn't already exist).
    pub fn get_all_authored_dbs(
        &self,
        dna_hash: &DnaHash,
    ) -> DatabaseResult<Vec<DbWrite<DbKindAuthored>>> {
        self.get_or_create_space_ref(dna_hash, |space| space.get_all_authored_dbs())
    }

    /// Get the authored database for this author if it already exists.
    pub fn get_authored_db_if_present(
        &self,
        dna_hash: &DnaHash,
        author: &AgentPubKey,
    ) -> DatabaseResult<Option<DbWrite<DbKindAuthored>>> {
        match self.map.share_ref(|spaces| spaces.get(dna_hash).cloned()) {
            Some(space) => space.get_authored_db_if_present(author),
            None => Ok(None),
        }
    }

    /// Get the dht database (this will create the space if it doesn't already exist).
    pub fn dht_db(&self, dna_hash: &DnaHash) -> DatabaseResult<DbWrite<DbKindDht>> {
        self.get_or_create_space_ref(dna_hash, |space| space.dht_db.clone())
    }

    /// Get the peer_meta_store database.
    pub fn peer_meta_store_db(
        &self,
        dna_hash: &DnaHash,
    ) -> DatabaseResult<DbWrite<DbKindPeerMetaStore>> {
        self.get_or_create_space_ref(dna_hash, |space| space.peer_meta_store_db.clone())
    }

    /// we are receiving a "publish" event from the network.
    #[cfg_attr(feature = "instrument", tracing::instrument(skip(self, ops)))]
    pub async fn handle_publish(&self, dna_hash: &DnaHash, ops: Vec<DhtOp>) -> ConductorResult<()> {
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

        incoming_dht_ops_workflow(space, trigger, ops).await?;

        Ok(())
    }

    /// Receive a publish countersign event from the network.
    #[cfg_attr(feature = "instrument", tracing::instrument(skip(self, ops)))]
    pub async fn handle_publish_countersign(
        &self,
        dna_hash: &DnaHash,
        op: ChainOp,
    ) -> ConductorResult<()> {
        let hash = DhtOpHash::with_data_sync(&op);

        let (workspace, trigger) = self.get_or_create_space_ref(dna_hash, |space| {
            (
                space.witnessing_workspace.clone(),
                self.queue_consumer_map
                    .witnessing_trigger(space.dna_hash.clone()),
            )
        })?;

        let trigger = match trigger {
            Some(t) => t,
            // If the workflow has not been spawned yet, we can't handle incoming messages.
            None => {
                tracing::warn!("No witnessing trigger yet for space: {}", dna_hash);
                return Ok(());
            }
        };

        receive_incoming_countersigning_ops(vec![(hash, op)], &workspace, trigger)?;

        Ok(())
    }
}

impl Space {
    fn new(
        dna_hash: Arc<DnaHash>,
        root_db_dir: PathBuf,
        db_sync_strategy: DbSyncStrategy,
        db_key: DbKey,
        db_max_readers: u16,
    ) -> DatabaseResult<Self> {
        let db_sync_level = match db_sync_strategy {
            DbSyncStrategy::Fast => DbSyncLevel::Off,
            DbSyncStrategy::Resilient => DbSyncLevel::Normal,
        };

        let (cache, dht_db, peer_meta_store_db, conductor_db) =
            tokio::task::block_in_place(|| {
                let cache = DbWrite::open_with_pool_config(
                    root_db_dir.as_ref(),
                    DbKindCache(dna_hash.clone()),
                    PoolConfig {
                        synchronous_level: db_sync_level,
                        key: db_key.clone(),
                        max_readers: db_max_readers,
                    },
                )?;
                let dht_db = DbWrite::open_with_pool_config(
                    root_db_dir.as_ref(),
                    DbKindDht(dna_hash.clone()),
                    PoolConfig {
                        synchronous_level: db_sync_level,
                        key: db_key.clone(),
                        max_readers: db_max_readers,
                    },
                )?;
                let peer_meta_store_db = DbWrite::open_with_pool_config(
                    root_db_dir.as_ref(),
                    DbKindPeerMetaStore(dna_hash.clone()),
                    PoolConfig {
                        synchronous_level: db_sync_level,
                        key: db_key.clone(),
                        max_readers: db_max_readers,
                    },
                )?;
                let conductor_db: DbWrite<DbKindConductor> = DbWrite::open_with_pool_config(
                    root_db_dir.as_ref(),
                    DbKindConductor,
                    PoolConfig {
                        synchronous_level: db_sync_level,
                        key: db_key.clone(),
                        max_readers: db_max_readers,
                    },
                )?;
                DatabaseResult::Ok((cache, dht_db, peer_meta_store_db, conductor_db))
            })?;

        let witnessing_workspace = WitnessingWorkspace::default();
        let incoming_op_hashes = IncomingOpHashes::default();
        let incoming_ops_batch = IncomingOpsBatch::default();
        let r = Self {
            dna_hash,
            cache_db: cache,
            authored_dbs: Arc::new(parking_lot::Mutex::new(HashMap::new())),
            dht_db,
            peer_meta_store_db,
            countersigning_workspaces: Default::default(),
            witnessing_workspace,
            incoming_op_hashes,
            incoming_ops_batch,
            conductor_db,
            root_db_dir: Arc::new(root_db_dir),
            db_key,
            db_max_readers,
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
            self.get_or_create_authored_db(author.clone())?,
            self.dht_db.clone(),
            keystore,
            author,
        )
        .await
    }

    /// Create a SourceChainWorkspace from this Space
    pub async fn source_chain_workspace(
        &self,
        keystore: MetaLairClient,
        author: AgentPubKey,
    ) -> ConductorResult<SourceChainWorkspace> {
        Ok(SourceChainWorkspace::new(
            self.get_or_create_authored_db(author.clone())?.clone(),
            self.dht_db.clone(),
            self.cache_db.clone(),
            keystore,
            author,
        )
        .await?)
    }

    /// Get or create the authored database for an agent in this space
    pub fn get_or_create_authored_db(
        &self,
        author: AgentPubKey,
    ) -> DatabaseResult<DbWrite<DbKindAuthored>> {
        match self.authored_dbs.lock().entry(author.clone()) {
            hash_map::Entry::Occupied(entry) => Ok(entry.get().clone()),
            hash_map::Entry::Vacant(entry) => {
                let db = tokio::task::block_in_place(|| {
                    DbWrite::open_with_pool_config(
                        self.root_db_dir.as_ref(),
                        DbKindAuthored(Arc::new(CellId::new((*self.dna_hash).clone(), author))),
                        PoolConfig {
                            synchronous_level: DbSyncLevel::Normal,
                            key: self.db_key.clone(),
                            max_readers: self.db_max_readers,
                        },
                    )
                })?;

                entry.insert(db.clone());
                Ok(db)
            }
        }
    }

    /// Get the authored database for an agent if it exists.
    pub fn get_authored_db_if_present(
        &self,
        author: &AgentPubKey,
    ) -> DatabaseResult<Option<DbWrite<DbKindAuthored>>> {
        Ok(self.authored_dbs.lock().get(author).cloned())
    }

    /// Gets authored databases for this space, for every author.
    pub fn get_all_authored_dbs(&self) -> Vec<DbWrite<DbKindAuthored>> {
        self.authored_dbs.lock().values().cloned().collect()
    }
}

/// Get the holochain conductor state
#[cfg_attr(feature = "instrument", tracing::instrument(skip_all))]
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
    /// Create a new test space
    pub async fn new(dna_hashes: impl IntoIterator<Item = DnaHash>) -> Self {
        let queue_consumer_map = QueueConsumerMap::new();
        Self::with_queue_consumer(dna_hashes, queue_consumer_map).await
    }

    /// Create a new test space with a queue consumer
    pub async fn with_queue_consumer(
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
            Arc::new(std::sync::Mutex::new(sodoken::LockedArray::from(
                b"passphrase".to_vec(),
            ))),
        )
        .await
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
    /// Create a new test space
    pub fn new(dna_hash: DnaHash) -> Self {
        let temp_dir = tempfile::Builder::new()
            .prefix("holochain-test-environments")
            .tempdir()
            .unwrap();

        Self {
            space: Space::new(
                Arc::new(dna_hash),
                temp_dir.path().to_path_buf(),
                Default::default(),
                Default::default(),
                ConductorConfig::default().db_max_readers,
            )
            .unwrap(),
            _temp_dir: temp_dir,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use holochain_conductor_api::conductor::ConductorConfig;
    use holochain_types::prelude::DnaHash;

    #[tokio::test(flavor = "multi_thread")]
    async fn db_max_readers_applied_to_pools() {
        let custom_max_readers = 24;

        let temp_dir = tempfile::Builder::new().tempdir().unwrap();

        let config_with_path = ConductorConfig {
            data_root_path: Some(temp_dir.path().to_path_buf().into()),
            db_max_readers: custom_max_readers,
            ..Default::default()
        };

        let spaces = Spaces::new(
            Arc::new(config_with_path),
            Arc::new(std::sync::Mutex::new(sodoken::LockedArray::from(
                b"passphrase".to_vec(),
            ))),
        )
        .await
        .unwrap();

        let dna_hash = DnaHash::from_raw_36(vec![0; 36]);
        let space = spaces.get_or_create_space(&dna_hash).unwrap();
        space
            .get_or_create_authored_db(AgentPubKey::from_raw_32(vec![0; 32]))
            .unwrap();

        // db_max_readers applied to space
        assert_eq!(space.db_max_readers, custom_max_readers);

        // db_max_readers applied to cache db
        assert_eq!(
            space.cache_db.connection_pool_max_size(),
            custom_max_readers as u32 + 1
        );

        // db_max_readers applied to dht db
        assert_eq!(
            space.dht_db.connection_pool_max_size(),
            custom_max_readers as u32 + 1
        );

        // db_max_readers applied to authored db
        assert_eq!(
            space
                .get_all_authored_dbs()
                .first()
                .unwrap()
                .connection_pool_max_size(),
            custom_max_readers as u32 + 1
        );
    }
}
