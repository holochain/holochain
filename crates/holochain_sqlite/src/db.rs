//! Functions dealing with obtaining and referencing singleton databases

use crate::{
    conn::{new_connection_pool, ConnectionPool, DbSyncLevel, PConn, DATABASE_HANDLES},
    prelude::*,
};
use derive_more::Into;
use futures::Future;
use holo_hash::DnaHash;
use holochain_zome_types::cell::CellId;
use kitsune_p2p::KitsuneSpace;
use parking_lot::Mutex;
use rusqlite::*;
use shrinkwraprs::Shrinkwrap;
use std::path::PathBuf;
use std::sync::Arc;
use std::{collections::HashMap, path::Path};
use tokio::{
    sync::{OwnedSemaphorePermit, Semaphore},
    task,
};

mod p2p_agent_store;
pub use p2p_agent_store::*;

mod p2p_metrics;
pub use p2p_metrics::*;

/// A read-only version of [DbWrite].
/// This environment can only generate read-only transactions, never read-write.
#[derive(Clone)]
pub struct DbRead {
    kind: DbKind,
    path: PathBuf,
    connection_pool: ConnectionPool,
    write_semaphore: Arc<Semaphore>,
    read_semaphore: Arc<Semaphore>,
}

impl DbRead {
    pub fn conn(&self) -> DatabaseResult<PConn> {
        self.connection_pooled()
    }

    #[deprecated = "remove this identity function"]
    pub fn inner(&self) -> Self {
        self.clone()
    }

    /// Accessor for the [DbKind] of the DbWrite
    pub fn kind(&self) -> &DbKind {
        &self.kind
    }

    /// The environment's path
    pub fn path(&self) -> &PathBuf {
        &self.path
    }

    /// Get a connection from the pool.
    /// TODO: We should eventually swap this for an async solution.
    fn connection_pooled(&self) -> DatabaseResult<PConn> {
        let now = std::time::Instant::now();
        let r = Ok(PConn::new(self.connection_pool.get()?, self.kind.clone()));
        let el = now.elapsed();
        if el.as_millis() > 20 {
            tracing::error!("Connection pool took {:?} to be free'd", el);
        }
        r
    }
}

/// The canonical representation of a (singleton) database.
/// The wrapper contains methods for managing transactions
/// and database connections,
// FIXME: this `derive_more::From` impl shouldn't be here!!
// But we have had this in the code for a long time...
#[derive(Clone, Shrinkwrap, Into, derive_more::From)]
pub struct DbWrite(DbRead);

impl DbWrite {
    /// Create or open an existing database reference,
    pub fn open(path_prefix: &Path, kind: DbKind) -> DatabaseResult<DbWrite> {
        Self::open_with_sync_level(path_prefix, kind, DbSyncLevel::default())
    }

    pub fn open_with_sync_level(
        path_prefix: &Path,
        kind: DbKind,
        sync_level: DbSyncLevel,
    ) -> DatabaseResult<DbWrite> {
        let path = path_prefix.join(kind.filename());
        if let Some(v) = DATABASE_HANDLES.get(&path) {
            Ok(v.clone())
        } else {
            let db = Self::new(path_prefix, kind, sync_level)?;
            DATABASE_HANDLES.insert_new(path, db.clone());
            Ok(db)
        }
    }

    pub(crate) fn new(
        path_prefix: &Path,
        kind: DbKind,
        sync_level: DbSyncLevel,
    ) -> DatabaseResult<Self> {
        let path = path_prefix.join(kind.filename());
        let parent = path
            .parent()
            .ok_or_else(|| DatabaseError::DatabaseMissing(path_prefix.to_owned()))?;
        if !parent.is_dir() {
            std::fs::create_dir_all(parent)
                .map_err(|_e| DatabaseError::DatabaseMissing(parent.to_owned()))?;
        }
        // Check if the database is valid and take the appropriate
        // action if it isn't.
        match Connection::open(&path)
            // For some reason calling pragma_update is necessary to prove the database file is valid.
            .and_then(|c| c.pragma_update(None, "synchronous", &"0".to_string()))
        {
            Ok(_) => (),
            // These are the two errors that can
            // occur if the database is not valid.
            err
            @
            Err(Error::SqliteFailure(
                rusqlite::ffi::Error {
                    code: ErrorCode::DatabaseCorrupt,
                    ..
                },
                ..,
            ))
            | err
            @
            Err(Error::SqliteFailure(
                rusqlite::ffi::Error {
                    code: ErrorCode::NotADatabase,
                    ..
                },
                ..,
            )) => {
                // Check if this database kind requires wiping.
                if kind.if_corrupt_wipe() {
                    std::fs::remove_file(&path)?;
                } else {
                    // If we don't wipe we need to return an error.
                    err?;
                }
            }
            // Another error has occurred when trying to open the db.
            Err(e) => return Err(e.into()),
        }

        // Now we know the database file is valid we can open a connection pool.
        let pool = new_connection_pool(&path, kind.clone(), sync_level);
        let mut conn = pool.get()?;
        // set to faster write-ahead-log mode
        conn.pragma_update(None, "journal_mode", &"WAL".to_string())?;
        crate::table::initialize_database(&mut conn, &kind)?;

        Ok(DbWrite(DbRead {
            write_semaphore: Self::get_write_semaphore(&kind),
            read_semaphore: Self::get_read_semaphore(&kind),
            kind,
            path,
            connection_pool: pool,
        }))
    }

    fn get_write_semaphore(kind: &DbKind) -> Arc<Semaphore> {
        static MAP: once_cell::sync::Lazy<Mutex<HashMap<DbKind, Arc<Semaphore>>>> =
            once_cell::sync::Lazy::new(|| Mutex::new(HashMap::new()));
        let mut map = MAP.lock();
        match map.get(kind) {
            Some(s) => s.clone(),
            None => {
                let s = Arc::new(Semaphore::new(1));
                map.insert(kind.clone(), s.clone());
                s
            }
        }
    }

    fn get_read_semaphore(kind: &DbKind) -> Arc<Semaphore> {
        static MAP: once_cell::sync::Lazy<Mutex<HashMap<DbKind, Arc<Semaphore>>>> =
            once_cell::sync::Lazy::new(|| Mutex::new(HashMap::new()));
        let mut map = MAP.lock();
        match map.get(kind) {
            Some(s) => s.clone(),
            None => {
                let s = Arc::new(Semaphore::new(num_read_threads()));
                map.insert(kind.clone(), s.clone());
                s
            }
        }
    }

    /// Create a unique db in a temp dir with no static management of the
    /// connection pool, useful for testing.
    #[cfg(any(test, feature = "test_utils"))]
    pub fn test(tmpdir: &tempdir::TempDir, kind: DbKind) -> DatabaseResult<Self> {
        Self::new(tmpdir.path(), kind, DbSyncLevel::default())
    }

    /// Remove the db and directory
    #[deprecated = "is this used?"]
    pub async fn remove(self) -> DatabaseResult<()> {
        if let Some(parent) = self.0.path.parent() {
            std::fs::remove_dir_all(parent)?;
        }
        Ok(())
    }
}

pub fn num_read_threads() -> usize {
    let num_cpus = num_cpus::get();
    let num_threads = num_cpus.checked_div(2).unwrap_or(0);
    std::cmp::max(num_threads, 4)
}

/// The various types of database, used to specify the list of databases to initialize
#[derive(Clone, Debug, PartialEq, Eq, Hash, derive_more::Display)]
pub enum DbKind {
    /// Specifies the environment used by each Cell
    Cell(CellId),
    /// Specifies the environment used by each Cache (one per dna).
    Cache(DnaHash),
    /// Specifies the environment used by a Conductor
    Conductor,
    /// Specifies the environment used to save wasm
    Wasm,
    /// State of the p2p network (one per space).
    P2pAgentStore(Arc<KitsuneSpace>),
    /// Metrics for peers on p2p network (one per space).
    P2pMetrics(Arc<KitsuneSpace>),
}

impl DbKind {
    /// Constuct a partial Path based on the kind
    pub fn filename(&self) -> PathBuf {
        let mut path: PathBuf = match self {
            DbKind::Cell(cell_id) => ["cell", &cell_id.to_string()].iter().collect(),
            DbKind::Cache(dna) => ["cache", &format!("cache-{}", dna)].iter().collect(),
            DbKind::Conductor => ["conductor", "conductor"].iter().collect(),
            DbKind::Wasm => ["wasm", "wasm"].iter().collect(),
            DbKind::P2pAgentStore(space) => ["p2p", &format!("p2p_agent_store-{}", space)]
                .iter()
                .collect(),
            DbKind::P2pMetrics(space) => {
                ["p2p", &format!("p2p_metrics-{}", space)].iter().collect()
            }
        };
        path.set_extension("sqlite3");
        path
    }

    /// Whether to wipe the database if it is corrupt.
    /// Some database it's safe to wipe them if they are corrupt because
    /// they can be refilled from the network. Other databases cannot
    /// be refilled and some manual intervention is required.
    fn if_corrupt_wipe(&self) -> bool {
        match self {
            // These databases can safely be wiped if they are corrupt.
            DbKind::Cache(_) => true,
            DbKind::P2pAgentStore(_) => true,
            DbKind::P2pMetrics(_) => true,
            // These databases cannot be safely wiped if they are corrupt.
            // TODO: When splitting the source chain and authority db the
            // authority db can be wiped but the source chain db cannot.
            DbKind::Cell(_) => false,
            DbKind::Wasm => false,
            DbKind::Conductor => false,
        }
    }
}

/// Implementors are able to create a new read-only DB transaction
pub trait ReadManager<'e> {
    /// Run a closure, passing in a new read-only transaction
    fn with_reader<E, R, F>(&'e mut self, f: F) -> Result<R, E>
    where
        E: From<DatabaseError>,
        F: 'e + FnOnce(Transaction) -> Result<R, E>;

    #[cfg(feature = "test_utils")]
    /// Same as with_reader, but with no Results: everything gets unwrapped
    fn with_reader_test<R, F>(&'e mut self, f: F) -> R
    where
        F: 'e + FnOnce(Transaction) -> R;
}

/// Implementors are able to create a new read-write DB transaction
pub trait WriteManager<'e> {
    /// Run a closure, passing in a mutable reference to a read-write
    /// transaction, and commit the transaction after the closure has run.
    /// If there is a SQLite error, recover from it and re-run the closure.
    // FIXME: B-01566: implement write failure detection
    fn with_commit_sync<E, R, F>(&'e mut self, f: F) -> Result<R, E>
    where
        E: From<DatabaseError>,
        F: 'e + FnOnce(&mut Transaction) -> Result<R, E>;

    // /// Get a raw read-write transaction for this environment.
    // /// It is preferable to use WriterManager::with_commit for database writes,
    // /// which can properly recover from and manage write failures
    // fn writer_unmanaged(&'e mut self) -> DatabaseResult<Writer<'e>>;

    #[cfg(feature = "test_utils")]
    fn with_commit_test<R, F>(&'e mut self, f: F) -> Result<R, DatabaseError>
    where
        F: 'e + FnOnce(&mut Transaction) -> R,
    {
        self.with_commit_sync(|w| DatabaseResult::Ok(f(w)))
    }
}

impl<'e> ReadManager<'e> for PConn {
    fn with_reader<E, R, F>(&'e mut self, f: F) -> Result<R, E>
    where
        E: From<DatabaseError>,
        F: 'e + FnOnce(Transaction) -> Result<R, E>,
    {
        let txn = self.transaction().map_err(DatabaseError::from)?;
        f(txn)
    }

    #[cfg(feature = "test_utils")]
    fn with_reader_test<R, F>(&'e mut self, f: F) -> R
    where
        F: 'e + FnOnce(Transaction) -> R,
    {
        self.with_reader(|r| DatabaseResult::Ok(f(r))).unwrap()
    }
}

impl DbRead {
    pub async fn async_reader<E, R, F>(&self, f: F) -> Result<R, E>
    where
        E: From<DatabaseError> + Send + 'static,
        F: FnOnce(Transaction) -> Result<R, E> + Send + 'static,
        R: Send + 'static,
    {
        let _g = self.acquire_reader_permit().await;
        let mut conn = self.conn()?;
        let r = task::spawn_blocking(move || conn.with_reader(f))
            .await
            .map_err(DatabaseError::from)?;
        r
    }

    async fn acquire_reader_permit(&self) -> OwnedSemaphorePermit {
        self.read_semaphore
            .clone()
            .acquire_owned()
            .await
            .expect("We don't ever close these semaphores")
    }
}

impl<'e> WriteManager<'e> for PConn {
    #[cfg(feature = "test_utils")]
    fn with_commit_sync<E, R, F>(&'e mut self, f: F) -> Result<R, E>
    where
        E: From<DatabaseError>,
        F: 'e + FnOnce(&mut Transaction) -> Result<R, E>,
    {
        let mut txn = self
            .transaction_with_behavior(TransactionBehavior::Exclusive)
            .map_err(DatabaseError::from)?;
        let result = f(&mut txn)?;
        txn.commit().map_err(DatabaseError::from)?;
        Ok(result)
    }
}

impl DbWrite {
    pub async fn async_commit<E, R, F>(&self, f: F) -> Result<R, E>
    where
        E: From<DatabaseError> + Send + 'static,
        F: FnOnce(&mut Transaction) -> Result<R, E> + Send + 'static,
        R: Send + 'static,
    {
        let _g = self.acquire_writer_permit().await;
        let mut conn = self.conn()?;
        let r = task::spawn_blocking(move || conn.with_commit_sync(f))
            .await
            .map_err(DatabaseError::from)?;
        r
    }

    /// If possible prefer async_commit as this is slower and can starve chained futures.
    pub async fn async_commit_in_place<E, R, F>(&self, f: F) -> Result<R, E>
    where
        E: From<DatabaseError>,
        F: FnOnce(&mut Transaction) -> Result<R, E>,
        R: Send,
    {
        let _g = self.acquire_writer_permit().await;
        let mut conn = self.conn()?;
        task::block_in_place(move || conn.with_commit_sync(f))
    }

    async fn acquire_writer_permit(&self) -> OwnedSemaphorePermit {
        self.0
            .write_semaphore
            .clone()
            .acquire_owned()
            .await
            .expect("We don't ever close these semaphores")
    }
}

#[derive(Debug)]
pub struct OptimisticRetryError<E: std::error::Error>(Vec<E>);

impl<E: std::error::Error> std::fmt::Display for OptimisticRetryError<E> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(
            f,
            "OptimisticRetryError had too many failures:\n{:#?}",
            self.0
        )
    }
}

impl<E: std::error::Error> std::error::Error for OptimisticRetryError<E> {}

pub async fn optimistic_retry_async<Func, Fut, T, E>(
    ctx: &str,
    mut f: Func,
) -> Result<T, OptimisticRetryError<E>>
where
    Func: FnMut() -> Fut,
    Fut: Future<Output = Result<T, E>>,
    E: std::error::Error,
{
    use tokio::time::Duration;
    const NUM_CONSECUTIVE_FAILURES: usize = 10;
    const RETRY_INTERVAL: Duration = Duration::from_millis(500);
    let mut errors = Vec::new();
    loop {
        match f().await {
            Ok(x) => return Ok(x),
            Err(err) => {
                tracing::error!(
                    "Error during optimistic_retry. Failures: {}/{}. Context: {}. Error: {:?}",
                    errors.len() + 1,
                    NUM_CONSECUTIVE_FAILURES,
                    ctx,
                    err
                );
                errors.push(err);
                if errors.len() >= NUM_CONSECUTIVE_FAILURES {
                    return Err(OptimisticRetryError(errors));
                }
            }
        }
        tokio::time::sleep(RETRY_INTERVAL).await;
    }
}
