//! Functions dealing with obtaining and referencing singleton databases

use crate::{
    conn::{new_connection_pool, ConnectionPool, DbSyncLevel, PConn, DATABASE_HANDLES},
    prelude::*,
};
use derive_more::Into;
use futures::Future;
use holo_hash::DnaHash;
use kitsune_p2p::KitsuneSpace;
use parking_lot::Mutex;
use rusqlite::*;
use shrinkwraprs::Shrinkwrap;
use std::sync::Arc;
use std::{collections::HashMap, path::Path};
use std::{path::PathBuf, sync::atomic::AtomicUsize};
use tokio::{
    sync::{OwnedSemaphorePermit, Semaphore},
    task,
};

mod p2p_agent_store;
pub use p2p_agent_store::*;

mod p2p_metrics;
pub use p2p_metrics::*;

#[async_trait::async_trait]
/// A trait for being generic over [`DbWrite`] and [`DbRead`] that
/// both implement read access.
pub trait ReadAccess<Kind: DbKindT>: Clone + Into<DbRead<Kind>> {
    /// Run an async read transaction on a background thread.
    async fn async_reader<E, R, F>(&self, f: F) -> Result<R, E>
    where
        E: From<DatabaseError> + Send + 'static,
        F: FnOnce(Transaction) -> Result<R, E> + Send + 'static,
        R: Send + 'static;

    /// Run an sync read transaction on a the current thread.
    fn sync_reader<E, R, F>(&self, f: F) -> Result<R, E>
    where
        E: From<DatabaseError>,
        F: FnOnce(Transaction) -> Result<R, E>;

    /// Access the kind of database.
    fn kind(&self) -> &Kind;
}

#[async_trait::async_trait]
impl<Kind: DbKindT> ReadAccess<Kind> for DbWrite<Kind> {
    async fn async_reader<E, R, F>(&self, f: F) -> Result<R, E>
    where
        E: From<DatabaseError> + Send + 'static,
        F: FnOnce(Transaction) -> Result<R, E> + Send + 'static,
        R: Send + 'static,
    {
        let db: &DbRead<Kind> = self.as_ref();
        DbRead::async_reader(db, f).await
    }

    fn sync_reader<E, R, F>(&self, f: F) -> Result<R, E>
    where
        E: From<DatabaseError>,
        F: FnOnce(Transaction) -> Result<R, E>,
    {
        let db: &DbRead<Kind> = self.as_ref();
        db.sync_reader(f)
    }

    fn kind(&self) -> &Kind {
        self.0.kind()
    }
}

#[async_trait::async_trait]
impl<Kind: DbKindT> ReadAccess<Kind> for DbRead<Kind> {
    async fn async_reader<E, R, F>(&self, f: F) -> Result<R, E>
    where
        E: From<DatabaseError> + Send + 'static,
        F: FnOnce(Transaction) -> Result<R, E> + Send + 'static,
        R: Send + 'static,
    {
        DbRead::async_reader(self, f).await
    }

    fn sync_reader<E, R, F>(&self, f: F) -> Result<R, E>
    where
        E: From<DatabaseError>,
        F: FnOnce(Transaction) -> Result<R, E>,
    {
        self.conn()?.with_reader(f)
    }

    fn kind(&self) -> &Kind {
        &self.kind
    }
}
/// A read-only version of [DbWrite].
/// This environment can only generate read-only transactions, never read-write.
#[derive(Clone)]
pub struct DbRead<Kind: DbKindT> {
    kind: Kind,
    path: PathBuf,
    connection_pool: ConnectionPool,
    write_semaphore: Arc<Semaphore>,
    read_semaphore: Arc<Semaphore>,
    max_readers: usize,
    num_readers: Arc<AtomicUsize>,
}

#[derive(Shrinkwrap)]
#[shrinkwrap(mutable)]
pub struct PConnGuard(#[shrinkwrap(main_field)] pub PConn, OwnedSemaphorePermit);

pub struct PConnPermit(OwnedSemaphorePermit);

pub trait PermittedConn {
    fn with_permit(&self, permit: PConnPermit) -> DatabaseResult<PConnGuard>;
}

impl<Kind: DbKindT> PermittedConn for DbRead<Kind> {
    fn with_permit(&self, permit: PConnPermit) -> DatabaseResult<PConnGuard> {
        Ok(PConnGuard(self.conn()?, permit.0))
    }
}

impl<Kind: DbKindT> PermittedConn for DbWrite<Kind> {
    fn with_permit(&self, permit: PConnPermit) -> DatabaseResult<PConnGuard> {
        Ok(PConnGuard(self.conn()?, permit.0))
    }
}

impl<Kind: DbKindT> DbRead<Kind> {
    pub fn conn(&self) -> DatabaseResult<PConn> {
        self.connection_pooled()
    }

    pub async fn conn_permit(&self) -> PConnPermit {
        let g = self.acquire_reader_permit().await;
        PConnPermit(g)
    }

    /// Accessor for the [DbKindT] of the DbWrite
    pub fn kind(&self) -> &Kind {
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
        let r = Ok(PConn::new(self.connection_pool.get()?));
        let el = now.elapsed();
        if el.as_millis() > 20 {
            tracing::error!("Connection pool took {:?} to be free'd", el);
        }
        r
    }

    pub async fn async_reader<E, R, F>(&self, f: F) -> Result<R, E>
    where
        E: From<DatabaseError> + Send + 'static,
        F: FnOnce(Transaction) -> Result<R, E> + Send + 'static,
        R: Send + 'static,
    {
        let waiting = self
            .num_readers
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        if waiting > self.max_readers {
            let s = tracing::info_span!("holochain_perf", kind = ?self.kind().kind());
            s.in_scope(|| {
                tracing::info!(
                    "Database read connection is saturated. Util {:.2}%",
                    waiting as f64 / self.max_readers as f64 * 100.0
                )
            });
        }
        let _g = self.acquire_reader_permit().await;
        self.num_readers
            .fetch_sub(1, std::sync::atomic::Ordering::Relaxed);
        let mut conn = self.conn()?;
        let r = tokio::task::spawn_blocking(move || conn.with_reader(f))
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

/// The canonical representation of a (singleton) database.
/// The wrapper contains methods for managing transactions
/// and database connections,
#[derive(Clone, Shrinkwrap, Into)]
pub struct DbWrite<Kind: DbKindT>(DbRead<Kind>);

impl<Kind: DbKindT + Send + Sync + 'static> DbWrite<Kind> {
    /// Create or open an existing database reference,
    pub fn open(path_prefix: &Path, kind: Kind) -> DatabaseResult<Self> {
        Self::open_with_sync_level(path_prefix, kind, DbSyncLevel::default())
    }

    pub async fn conn_write_permit(&self) -> PConnPermit {
        let g = self.acquire_writer_permit().await;
        PConnPermit(g)
    }

    pub fn open_with_sync_level(
        path_prefix: &Path,
        kind: Kind,
        sync_level: DbSyncLevel,
    ) -> DatabaseResult<Self> {
        DATABASE_HANDLES.get_or_insert(&kind, path_prefix, |kind| {
            Self::new(Some(path_prefix), kind, sync_level)
        })
    }

    pub(crate) fn new(
        path_prefix: Option<&Path>,
        kind: Kind,
        sync_level: DbSyncLevel,
    ) -> DatabaseResult<Self> {
        let path = match path_prefix {
            Some(path_prefix) => {
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
                    .and_then(|mut c| {
                        crate::conn::initialize_connection(&mut c, sync_level)?;
                        c.pragma_update(None, "synchronous", "0".to_string())
                    }) {
                    Ok(_) => (),
                    // These are the two errors that can
                    // occur if the database is not valid.
                    err @ Err(Error::SqliteFailure(
                        rusqlite::ffi::Error {
                            code: ErrorCode::DatabaseCorrupt,
                            ..
                        },
                        ..,
                    ))
                    | err @ Err(Error::SqliteFailure(
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
                Some(path)
            }
            None => None,
        };

        // Now we know the database file is valid we can open a connection pool.
        let pool = new_connection_pool(path.as_ref().map(|p| p.as_ref()), sync_level);
        let mut conn = pool.get()?;
        // set to faster write-ahead-log mode
        conn.pragma_update(None, "journal_mode", "WAL".to_string())?;
        crate::table::initialize_database(&mut conn, kind.kind())?;

        Ok(DbWrite(DbRead {
            write_semaphore: Self::get_write_semaphore(kind.kind()),
            read_semaphore: Self::get_read_semaphore(kind.kind()),
            max_readers: num_read_threads(),
            num_readers: Arc::new(AtomicUsize::new(0)),
            kind,
            path: path.unwrap_or_default(),
            connection_pool: pool,
        }))
    }

    fn get_write_semaphore(kind: DbKind) -> Arc<Semaphore> {
        static MAP: once_cell::sync::Lazy<Mutex<HashMap<DbKind, Arc<Semaphore>>>> =
            once_cell::sync::Lazy::new(|| Mutex::new(HashMap::new()));
        MAP.lock()
            .entry(kind)
            .or_insert_with(|| Arc::new(Semaphore::new(1)))
            .clone()
    }

    fn get_read_semaphore(kind: DbKind) -> Arc<Semaphore> {
        static MAP: once_cell::sync::Lazy<Mutex<HashMap<DbKind, Arc<Semaphore>>>> =
            once_cell::sync::Lazy::new(|| Mutex::new(HashMap::new()));
        MAP.lock()
            .entry(kind)
            .or_insert_with(|| Arc::new(Semaphore::new(num_read_threads())))
            .clone()
    }

    /// Create a unique db in a temp dir with no static management of the
    /// connection pool, useful for testing.
    #[cfg(any(test, feature = "test_utils"))]
    pub fn test(path: &Path, kind: Kind) -> DatabaseResult<Self> {
        Self::new(Some(path), kind, DbSyncLevel::default())
    }

    #[cfg(any(test, feature = "test_utils"))]
    pub fn test_in_mem(kind: Kind) -> DatabaseResult<Self> {
        Self::new(None, kind, DbSyncLevel::default())
    }

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

    #[cfg(any(test, feature = "test_utils"))]
    pub fn test_commit<R, F>(&self, f: F) -> R
    where
        F: FnOnce(&mut Transaction) -> R,
    {
        let mut conn = self.conn().expect("Failed to open connection");
        conn.with_commit_test(f)
            .expect("Database transaction failed")
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

pub fn num_read_threads() -> usize {
    let num_cpus = num_cpus::get();
    let num_threads = num_cpus.checked_div(2).unwrap_or(0);
    std::cmp::max(num_threads, 4)
}

/// The various types of database, used to specify the list of databases to initialize
#[derive(Clone, Debug, PartialEq, Eq, Hash, derive_more::Display)]
pub enum DbKind {
    /// Specifies the environment used for authoring data by all cells on the same [`DnaHash`].
    Authored(Arc<DnaHash>),
    /// Specifies the environment used for dht data by all cells on the same [`DnaHash`].
    Dht(Arc<DnaHash>),
    /// Specifies the environment used by each Cache (one per dna).
    Cache(Arc<DnaHash>),
    /// Specifies the environment used by a Conductor
    Conductor,
    /// Specifies the environment used to save wasm
    Wasm,
    /// State of the p2p network (one per space).
    P2pAgentStore(Arc<KitsuneSpace>),
    /// Metrics for peers on p2p network (one per space).
    P2pMetrics(Arc<KitsuneSpace>),
}
pub trait DbKindT: Clone + Send + Sync + 'static {
    fn kind(&self) -> DbKind;
    /// Constuct a partial Path based on the kind
    fn filename(&self) -> PathBuf {
        let mut path = self.filename_inner();
        path.set_extension("sqlite3");
        path
    }
    /// The above provided `filename` method attaches the .sqlite3 extension.
    /// Implement this to provide the front part of the database filename.
    fn filename_inner(&self) -> PathBuf;
    /// Whether to wipe the database if it is corrupt.
    /// Some database it's safe to wipe them if they are corrupt because
    /// they can be refilled from the network. Other databases cannot
    /// be refilled and some manual intervention is required.
    fn if_corrupt_wipe(&self) -> bool;
}

pub trait DbKindOp {}

#[derive(Clone, Debug, PartialEq, Eq, Hash, derive_more::Display)]
/// Specifies the environment used for authoring data by all cells on the same [`DnaHash`].
pub struct DbKindAuthored(pub Arc<DnaHash>);

#[derive(Clone, Debug, PartialEq, Eq, Hash, derive_more::Display)]
/// Specifies the environment used for dht data by all cells on the same [`DnaHash`].
pub struct DbKindDht(pub Arc<DnaHash>);

#[derive(Clone, Debug, PartialEq, Eq, Hash, derive_more::Display)]
/// Specifies the environment used by each Cache (one per dna).
pub struct DbKindCache(pub Arc<DnaHash>);

#[derive(Clone, Debug, PartialEq, Eq, Hash, derive_more::Display)]
/// Specifies the environment used by a Conductor
pub struct DbKindConductor;

#[derive(Clone, Debug, PartialEq, Eq, Hash, derive_more::Display)]
/// Specifies the environment used to witness nonces.
pub struct DbKindNonce;

#[derive(Clone, Debug, PartialEq, Eq, Hash, derive_more::Display)]
/// Specifies the environment used to save wasm
pub struct DbKindWasm;

#[derive(Clone, Debug, PartialEq, Eq, Hash, derive_more::Display)]
/// State of the p2p network (one per space).
pub struct DbKindP2pAgents(pub Arc<KitsuneSpace>);

#[derive(Clone, Debug, PartialEq, Eq, Hash, derive_more::Display)]
/// Metrics for peers on p2p network (one per space).
pub struct DbKindP2pMetrics(pub Arc<KitsuneSpace>);

impl DbKindT for DbKindAuthored {
    fn kind(&self) -> DbKind {
        DbKind::Authored(self.0.clone())
    }

    fn filename_inner(&self) -> PathBuf {
        ["authored", &format!("authored-{}", self.0)]
            .iter()
            .collect()
    }

    fn if_corrupt_wipe(&self) -> bool {
        false
    }
}

impl DbKindOp for DbKindAuthored {}

impl DbKindAuthored {
    pub fn dna_hash(&self) -> &DnaHash {
        &self.0
    }
    pub fn to_dna_hash(&self) -> Arc<DnaHash> {
        self.0.clone()
    }
}

impl DbKindT for DbKindDht {
    fn kind(&self) -> DbKind {
        DbKind::Dht(self.0.clone())
    }

    fn filename_inner(&self) -> PathBuf {
        ["dht", &format!("dht-{}", self.0)].iter().collect()
    }

    fn if_corrupt_wipe(&self) -> bool {
        true
    }
}

impl DbKindOp for DbKindDht {}

impl DbKindDht {
    pub fn dna_hash(&self) -> &DnaHash {
        &self.0
    }
    pub fn to_dna_hash(&self) -> Arc<DnaHash> {
        self.0.clone()
    }
}

impl DbKindT for DbKindCache {
    fn kind(&self) -> DbKind {
        DbKind::Cache(self.0.clone())
    }

    fn filename_inner(&self) -> PathBuf {
        ["cache", &format!("cache-{}", self.0)].iter().collect()
    }

    fn if_corrupt_wipe(&self) -> bool {
        true
    }
}

impl DbKindCache {
    pub fn dna_hash(&self) -> &DnaHash {
        &self.0
    }
    pub fn to_dna_hash(&self) -> Arc<DnaHash> {
        self.0.clone()
    }
}

impl DbKindOp for DbKindCache {}

impl DbKindT for DbKindConductor {
    fn kind(&self) -> DbKind {
        DbKind::Conductor
    }

    fn filename_inner(&self) -> PathBuf {
        ["conductor", "conductor"].iter().collect()
    }

    fn if_corrupt_wipe(&self) -> bool {
        false
    }
}

impl DbKindT for DbKindWasm {
    fn kind(&self) -> DbKind {
        DbKind::Wasm
    }

    fn filename_inner(&self) -> PathBuf {
        ["wasm", "wasm"].iter().collect()
    }

    fn if_corrupt_wipe(&self) -> bool {
        false
    }
}

impl DbKindT for DbKindP2pAgents {
    fn kind(&self) -> DbKind {
        DbKind::P2pAgentStore(self.0.clone())
    }

    fn filename_inner(&self) -> PathBuf {
        ["p2p", &format!("p2p_agent_store-{}", self.0)]
            .iter()
            .collect()
    }

    fn if_corrupt_wipe(&self) -> bool {
        true
    }
}

impl DbKindT for DbKindP2pMetrics {
    fn kind(&self) -> DbKind {
        DbKind::P2pMetrics(self.0.clone())
    }

    fn filename_inner(&self) -> PathBuf {
        ["p2p", &format!("p2p_metrics-{}", self.0)].iter().collect()
    }

    fn if_corrupt_wipe(&self) -> bool {
        true
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

    #[cfg(any(test, feature = "test_utils"))]
    fn with_commit_test<R, F>(&'e mut self, f: F) -> Result<R, DatabaseError>
    where
        F: 'e + FnOnce(&mut Transaction) -> R,
    {
        self.with_commit_sync(|w| DatabaseResult::Ok(f(w)))
    }
}

impl<'e> PConn {
    fn with_reader<E, R, F>(&'e mut self, f: F) -> Result<R, E>
    where
        E: From<DatabaseError>,
        F: 'e + FnOnce(Transaction) -> Result<R, E>,
    {
        let start = std::time::Instant::now();
        let txn = self.transaction().map_err(DatabaseError::from)?;
        if start.elapsed().as_millis() > 100 {
            let s = tracing::debug_span!("timing_1");
            s.in_scope(
                || tracing::debug!(file = %file!(), line = %line!(), time = ?start.elapsed()),
            );
        }
        f(txn)
    }

    #[cfg(feature = "test_utils")]
    pub fn with_reader_test<R, F>(&'e mut self, f: F) -> R
    where
        F: 'e + FnOnce(Transaction) -> R,
    {
        self.with_reader(|r| DatabaseResult::Ok(f(r))).unwrap()
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
