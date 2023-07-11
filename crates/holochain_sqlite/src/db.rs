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
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Instant;
use std::{collections::HashMap, path::Path};
use std::{path::PathBuf, sync::atomic::AtomicUsize};
use tokio::sync::{OwnedSemaphorePermit, Semaphore};

mod p2p_agent_store;
pub use p2p_agent_store::*;

mod p2p_metrics;
pub use p2p_metrics::*;

static ACQUIRE_TIMEOUT_MS: AtomicU64 = AtomicU64::new(10_000);

#[async_trait::async_trait]
/// A trait for being generic over [`DbWrite`] and [`DbRead`] that
/// both implement read access.
pub trait ReadAccess<Kind: DbKindT>: Clone + Into<DbRead<Kind>> {
    /// Run an async read transaction on a background thread.
    async fn read_async<E, R, F>(&self, f: F) -> Result<R, E>
    where
        E: From<DatabaseError> + Send + 'static,
        F: FnOnce(Transaction) -> Result<R, E> + Send + 'static,
        R: Send + 'static;

    /// Access the kind of database.
    fn kind(&self) -> &Kind;
}

#[async_trait::async_trait]
impl<Kind: DbKindT> ReadAccess<Kind> for DbWrite<Kind> {
    async fn read_async<E, R, F>(&self, f: F) -> Result<R, E>
    where
        E: From<DatabaseError> + Send + 'static,
        F: FnOnce(Transaction) -> Result<R, E> + Send + 'static,
        R: Send + 'static,
    {
        let db: &DbRead<Kind> = self.as_ref();
        DbRead::read_async(db, f).await
    }

    fn kind(&self) -> &Kind {
        self.0.kind()
    }
}

#[async_trait::async_trait]
impl<Kind: DbKindT> ReadAccess<Kind> for DbRead<Kind> {
    async fn read_async<E, R, F>(&self, f: F) -> Result<R, E>
    where
        E: From<DatabaseError> + Send + 'static,
        F: FnOnce(Transaction) -> Result<R, E> + Send + 'static,
        R: Send + 'static,
    {
        DbRead::read_async(self, f).await
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
    long_read_semaphore: Arc<Semaphore>,
    statement_trace_fn: Option<fn(&str)>,
    max_readers: usize,
    num_readers: Arc<AtomicUsize>,
}

impl<Kind: DbKindT> std::fmt::Debug for DbRead<Kind> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DbRead")
            .field("kind", &self.kind)
            .field("path", &self.path)
            .field("max_readers", &self.max_readers)
            .field("num_readers", &self.num_readers)
            .finish()
    }
}

#[derive(Shrinkwrap)]
#[shrinkwrap(mutable)]
pub struct PConnGuard(#[shrinkwrap(main_field)] pub PConn, OwnedSemaphorePermit);

/// This type exists to hand out connections that can only be used for running transactions
pub struct PTxnGuard(PConnGuard, Instant);

impl PTxnGuard {
    /// Start a new transaction on the inner connection held by this txn guard.
    pub fn transaction(&mut self) -> DatabaseResult<Transaction<'_>> {
        Ok(self.0.transaction()?)
    }
}

impl From<PConnGuard> for PTxnGuard {
    fn from(value: PConnGuard) -> Self {
        PTxnGuard(value, Instant::now())
    }
}

impl Drop for PTxnGuard {
    fn drop(&mut self) {
        // TODO record histogram rather than logging a warning on a fixed threshold
        let elapsed_millis = self.1.elapsed().as_millis();
        if elapsed_millis > 50 {
            tracing::warn!("PTxnGuard was held for {:?}ms", elapsed_millis);
        }
    }
}

pub struct PConnPermit(OwnedSemaphorePermit);

// #[deprecated(
//     since = "0.3.0-beta-dev.5",
//     note = "Use `read_async` or `write_async` instead"
// )]
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
    // TODO should not be public, it is only used internally and by tests. Tests should just move to use the read/write trait methods.
    // #[deprecated(
    //     since = "0.3.0-beta-dev.5",
    //     note = "Use `read_async` or `write_async` instead"
    // )]
    #[cfg(feature = "test_utils")]
    pub fn conn(&self) -> DatabaseResult<PConn> {
        self.get_connection_from_pool()
    }

    // TODO don't get a permit directly because it must not be held for too long, use the read/write trait methods.
    // #[deprecated(
    //     since = "0.3.0-beta-dev.5",
    //     note = "Use `read_async` or `write_async` instead"
    // )]
    pub async fn conn_permit<E>(&self) -> Result<PConnPermit, E>
    where
        E: From<DatabaseError> + Send + 'static,
    {
        let g = Self::acquire_reader_permit(self.read_semaphore.clone()).await?;
        Ok(PConnPermit(g))
    }

    /// Accessor for the [DbKindT] of the DbWrite
    pub fn kind(&self) -> &Kind {
        &self.kind
    }

    /// The environment's path
    pub fn path(&self) -> &PathBuf {
        &self.path
    }

    /// Execute a read closure on the database by acquiring a connection from the pool, starting a new transaction and
    /// running the closure with that transaction.
    ///
    /// Note that it is not enforced that your closure runs read-only operations or that it finishes quickly so it is
    /// up to the caller to use this function as intended.
    pub async fn read_async<E, R, F>(&self, f: F) -> Result<R, E>
    where
        E: From<DatabaseError> + Send + 'static,
        F: FnOnce(Transaction) -> Result<R, E> + Send + 'static,
        R: Send + 'static,
    {
        let mut conn = self
            .checkout_connection(self.read_semaphore.clone())
            .await?;
        let r = tokio::task::spawn_blocking(move || conn.execute_in_read_txn(f))
            .await
            .map_err(DatabaseError::from)?;
        r
    }

    /// Intended to be used for transactions that need to be kept open for a longer period of time than just running a
    /// sequence of reads using `read_async`. You should default to `read_async` and only call this if you have a good
    /// reason.
    ///
    /// A valid reason for this is holding read transactions across multiple databases as part of a cascade query.
    pub async fn get_read_txn(&self) -> DatabaseResult<PTxnGuard> {
        let conn = self
            .checkout_connection(self.long_read_semaphore.clone())
            .await?;
        Ok(conn.into())
    }

    async fn checkout_connection(&self, semaphore: Arc<Semaphore>) -> DatabaseResult<PConnGuard> {
        let waiting = self.num_readers.fetch_add(1, Ordering::Relaxed);
        if waiting > self.max_readers {
            let s = tracing::info_span!("holochain_perf", kind = ?self.kind().kind());
            s.in_scope(|| {
                tracing::info!(
                    "Database read connection is saturated. Util {:.2}%",
                    waiting as f64 / self.max_readers as f64 * 100.0
                )
            });
        }

        let permit = Self::acquire_reader_permit(semaphore).await?;
        self.num_readers.fetch_sub(1, Ordering::Relaxed);

        let mut conn = self.get_connection_from_pool()?;
        if self.statement_trace_fn.is_some() {
            conn.trace(self.statement_trace_fn);
        }

        Ok(PConnGuard(conn, permit))
    }

    /// Get a connection from the pool.
    /// TODO: We should eventually swap this for an async solution.
    fn get_connection_from_pool(&self) -> DatabaseResult<PConn> {
        let now = std::time::Instant::now();
        let r = Ok(PConn::new(self.connection_pool.get()?));
        let el = now.elapsed();
        if el.as_millis() > 20 {
            tracing::error!("Connection pool took {:?} to be free'd", el);
        }
        r
    }

    async fn acquire_reader_permit(
        semaphore: Arc<Semaphore>,
    ) -> DatabaseResult<OwnedSemaphorePermit> {
        match tokio::time::timeout(
            std::time::Duration::from_millis(ACQUIRE_TIMEOUT_MS.load(Ordering::Acquire)),
            semaphore.acquire_owned(),
        )
        .await
        {
            Ok(Ok(s)) => Ok(s),
            Ok(Err(e)) => {
                tracing::error!("Semaphore should not be closed but got an error while acquiring a permit, {:?}", e);
                Err(DatabaseError::Other(e.into()))
            }
            Err(e) => Err(DatabaseError::Timeout(e)),
        }
    }
}

/// The canonical representation of a (singleton) database.
/// The wrapper contains methods for managing transactions
/// and database connections,
#[derive(Clone, Debug, Shrinkwrap, Into)]
pub struct DbWrite<Kind: DbKindT>(DbRead<Kind>);

impl<Kind: DbKindT + Send + Sync + 'static> DbWrite<Kind> {
    /// Create or open an existing database reference,
    pub fn open(path_prefix: &Path, kind: Kind) -> DatabaseResult<Self> {
        Self::open_with_sync_level(path_prefix, kind, DbSyncLevel::default())
    }

    pub fn open_with_sync_level(
        path_prefix: &Path,
        kind: Kind,
        sync_level: DbSyncLevel,
    ) -> DatabaseResult<Self> {
        DATABASE_HANDLES.get_or_insert(&kind, path_prefix, |kind| {
            Self::new(Some(path_prefix), kind, sync_level, None)
        })
    }

    pub fn new(
        path_prefix: Option<&Path>,
        kind: Kind,
        sync_level: DbSyncLevel,
        statement_trace_fn: Option<fn(&str)>,
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
            long_read_semaphore: Self::get_long_read_semaphore(kind.kind()),
            max_readers: num_read_threads() * 2,
            num_readers: Arc::new(AtomicUsize::new(0)),
            kind,
            path: path.unwrap_or_default(),
            connection_pool: pool,
            statement_trace_fn,
        }))
    }

    pub async fn write_async<E, R, F>(&self, f: F) -> Result<R, E>
    where
        E: From<DatabaseError> + Send + 'static,
        F: FnOnce(&mut Transaction) -> Result<R, E> + Send + 'static,
        R: Send + 'static,
    {
        let _g = tokio::time::timeout(
            std::time::Duration::from_secs(10),
            self.acquire_writer_permit(),
        )
        .await
        .map_err(DatabaseError::from)?;

        let mut conn = self.get_connection_from_pool()?;
        let r = tokio::task::spawn_blocking(move || conn.execute_in_exclusive_rw_txn(f))
            .await
            .map_err(DatabaseError::from)?;
        r
    }

    pub fn available_writer_count(&self) -> usize {
        self.write_semaphore.available_permits()
    }

    pub fn available_reader_count(&self) -> usize {
        self.read_semaphore.available_permits()
    }

    async fn acquire_writer_permit(&self) -> OwnedSemaphorePermit {
        self.0
            .write_semaphore
            .clone()
            .acquire_owned()
            .await
            .expect("We don't ever close these semaphores")
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

    fn get_long_read_semaphore(kind: DbKind) -> Arc<Semaphore> {
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
        Self::new(Some(path), kind, DbSyncLevel::default(), None)
    }

    #[cfg(any(test, feature = "test_utils"))]
    pub fn test_in_mem(kind: Kind) -> DatabaseResult<Self> {
        Self::new(None, kind, DbSyncLevel::default(), None)
    }

    #[cfg(any(test, feature = "test_utils"))]
    pub fn test_commit<R, F>(&self, f: F) -> R
    where
        F: FnOnce(&mut Transaction) -> R,
    {
        let mut conn = self.conn().expect("Failed to open connection");
        conn.execute_in_exclusive_rw_txn(|w| DatabaseResult::Ok(f(w)))
            .expect("Database transaction failed")
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
    #[cfg(feature = "test_utils")]
    Test(String),
}

pub trait DbKindT: Clone + std::fmt::Debug + Send + Sync + 'static {
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

impl<'e> PConn {
    fn execute_in_read_txn<E, R, F>(&'e mut self, f: F) -> Result<R, E>
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
        // TODO It would be possible to prevent the transaction from calling commit here if we passed a reference instead of a move.
        f(txn)
    }

    /// Run a closure, passing in a mutable reference to a read-write
    /// transaction, and commit the transaction after the closure has run.
    /// If there is a SQLite error, recover from it and re-run the closure.
    // FIXME: B-01566: implement write failure detection
    fn execute_in_exclusive_rw_txn<E, R, F>(&'e mut self, f: F) -> Result<R, E>
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

#[cfg(feature = "test_utils")]
pub fn set_acquire_timeout(timeout_ms: u64) {
    ACQUIRE_TIMEOUT_MS.store(timeout_ms, Ordering::Relaxed);
}
