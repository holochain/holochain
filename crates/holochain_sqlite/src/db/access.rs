use crate::db::conn::PConn;
use crate::db::databases::DATABASE_HANDLES;
use crate::db::guard::{PConnGuard, PTxnGuard};
use crate::db::kind::{DbKind, DbKindT};
use crate::db::pool::{
    initialize_connection, new_connection_pool, num_read_threads, ConnectionPool, DbSyncLevel,
};
use crate::error::{DatabaseError, DatabaseResult};
use derive_more::Into;
use parking_lot::Mutex;
use rusqlite::*;
use shrinkwraprs::Shrinkwrap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Instant;
use std::{collections::HashMap, path::Path};
use std::{path::PathBuf, sync::atomic::AtomicUsize};
use tokio::sync::{OwnedSemaphorePermit, Semaphore};

use super::metrics::{create_connection_use_time_metric, create_pool_usage_metric, UseTimeMetric};

static ACQUIRE_TIMEOUT_MS: AtomicU64 = AtomicU64::new(10_000);
static THREAD_ACQUIRE_TIMEOUT_MS: AtomicU64 = AtomicU64::new(30_000);

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
    use_time_metric: UseTimeMetric,
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

impl<Kind: DbKindT> DbRead<Kind> {
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

        // Once sync code starts in the spawn_blocking it cannot be cancelled BUT if we've run out of threads to execute blocking work on then
        // this timeout should prevent the caller being blocked by this await that may not finish.
        tokio::time::timeout(std::time::Duration::from_millis(THREAD_ACQUIRE_TIMEOUT_MS.load(Ordering::Acquire)), tokio::task::spawn_blocking(move || {
                conn.execute_in_read_txn(f)
            })).await.map_err(|e| {
                tracing::error!("Failed to claim a thread to run the database read transaction. It's likely that the program is out of threads.");
                DatabaseError::from(e)
            })?.map_err(DatabaseError::from)?
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

        Ok(PConnGuard::new(conn, permit, self.use_time_metric.clone()))
    }

    /// Get a connection from the pool.
    /// TODO: We should eventually swap this for an async solution.
    fn get_connection_from_pool(&self) -> DatabaseResult<PConn> {
        let now = Instant::now();
        let r = Ok(PConn::new(self.connection_pool.get()?));
        let el = now.elapsed();
        if el.as_millis() > 20 {
            // TODO Convert to a metric
            tracing::info!("Connection pool took {:?} to be freed", el);
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

    #[cfg(all(any(test, feature = "test_utils"), not(loom)))]
    pub fn test_read<R, F>(&self, f: F) -> R
    where
        F: FnOnce(Transaction) -> R + Send + 'static,
        R: Send + 'static,
    {
        holochain_util::tokio_helper::block_forever_on(async {
            self.read_async(move |txn| -> DatabaseResult<R> { Ok(f(txn)) })
                .await
                .unwrap()
        })
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
                match Self::check_database_file(&path, sync_level) {
                    Ok(path) => path,
                    // These are the two errors that can
                    // occur if the database is not valid.
                    err @ Err(Error::SqliteFailure(
                        ffi::Error {
                            code: ErrorCode::DatabaseCorrupt,
                            ..
                        },
                        ..,
                    ))
                    | err @ Err(Error::SqliteFailure(
                        ffi::Error {
                            code: ErrorCode::NotADatabase,
                            ..
                        },
                        ..,
                    )) => {
                        // Check if the database might be unencrypted.
                        if "true"
                            == std::env::var("HOLOCHAIN_MIGRATE_UNENCRYPTED")
                                .unwrap_or_default()
                                .as_str()
                        {
                            #[cfg(feature = "sqlite-encrypted")]
                            encrypt_unencrypted_database(&path)?;
                        }
                        // Check if this database kind requires wiping.
                        else if kind.if_corrupt_wipe() {
                            std::fs::remove_file(&path)?;
                        } else {
                            // If we don't wipe we need to return an error.
                            err?;
                        }

                        // Now that we've taken the appropriate action we can try again.
                        match Self::check_database_file(&path, sync_level) {
                            Ok(path) => path,
                            Err(e) => return Err(e.into()),
                        }
                    }
                    // Another error has occurred when trying to open the db.
                    Err(e) => return Err(e.into()),
                }
            }
            None => None,
        };

        // Now we know the database file is valid we can open a connection pool.
        let pool = new_connection_pool(path.as_ref().map(|p| p.as_ref()), sync_level);
        let mut conn = pool.get()?;
        // set to faster write-ahead-log mode
        conn.pragma_update(None, "journal_mode", "WAL".to_string())?;
        crate::table::initialize_database(&mut conn, kind.kind())?;

        let use_time_metric = create_connection_use_time_metric(kind.kind());

        let db_read = DbRead {
            write_semaphore: Self::get_write_semaphore(kind.kind()),
            read_semaphore: Self::get_read_semaphore(kind.kind()),
            long_read_semaphore: Self::get_long_read_semaphore(kind.kind()),
            max_readers: num_read_threads() * 2,
            num_readers: Arc::new(AtomicUsize::new(0)),
            kind: kind.clone(),
            path: path.unwrap_or_default(),
            connection_pool: pool,
            statement_trace_fn,
            use_time_metric,
        };

        create_pool_usage_metric(
            kind.kind(),
            vec![
                db_read.write_semaphore.clone(),
                db_read.read_semaphore.clone(),
                db_read.long_read_semaphore.clone(),
            ],
        );

        Ok(DbWrite(db_read))
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

        // Once sync code starts in the spawn_blocking it cannot be cancelled BUT if we've run out of threads to execute blocking work on then
        // this timeout should prevent the caller being blocked by this await that may not finish.
        tokio::time::timeout(std::time::Duration::from_millis(THREAD_ACQUIRE_TIMEOUT_MS.load(Ordering::Acquire)), tokio::task::spawn_blocking(move || {
            conn.execute_in_exclusive_rw_txn(f)
        })).await.map_err(|e| {
            tracing::error!("Failed to claim a thread to run the database write transaction. It's likely that the program is out of threads.");
            DatabaseError::from(e)
        })?.map_err(DatabaseError::from)?
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

    fn check_database_file(
        path: &Path,
        sync_level: DbSyncLevel,
    ) -> rusqlite::Result<Option<PathBuf>> {
        Connection::open(path)
            // For some reason calling pragma_update is necessary to prove the database file is valid.
            .and_then(|mut c| {
                initialize_connection(&mut c, sync_level)?;
                c.pragma_update(None, "synchronous", "0".to_string())?;
                Ok(c.path().map(PathBuf::from))
            })
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

    #[cfg(all(any(test, feature = "test_utils"), not(loom)))]
    pub fn test_write<R, F>(&self, f: F) -> R
    where
        F: FnOnce(&mut Transaction) -> R + Send + 'static,
        R: Send + 'static,
    {
        holochain_util::tokio_helper::block_forever_on(async {
            self.write_async(|txn| -> DatabaseResult<R> { Ok(f(txn)) })
                .await
                .unwrap()
        })
    }
}

// The method for this function is taken from https://discuss.zetetic.net/t/how-to-encrypt-a-plaintext-sqlite-database-to-use-sqlcipher-and-avoid-file-is-encrypted-or-is-not-a-database-errors/868
#[cfg(feature = "sqlite-encrypted")]
pub fn encrypt_unencrypted_database(path: &Path) -> DatabaseResult<()> {
    // e.g. conductor/conductor.sqlite3 -> conductor/conductor-encrypted.sqlite3
    let encrypted_path = path
        .parent()
        .ok_or_else(|| DatabaseError::DatabaseMissing(path.to_owned()))?
        .join(
            path.file_stem()
                .and_then(|s| s.to_str())
                .ok_or_else(|| DatabaseError::DatabaseMissing(path.to_owned()))?
                .to_string()
                + "-encrypted."
                + path
                    .extension()
                    .and_then(|s| s.to_str())
                    .ok_or_else(|| DatabaseError::DatabaseMissing(path.to_owned()))?,
        );

    tracing::warn!(
        "Attempting encryption of unencrypted database: {:?} -> {:?}",
        path,
        encrypted_path
    );

    // Migrate the database
    {
        let conn = Connection::open(path)?;

        // Ensure everything in the WAL is written to the main database
        conn.execute("VACUUM", ())?;

        // Start an exclusive transaction to avoid anybody writing to the database while we're migrating it
        conn.execute("BEGIN EXCLUSIVE", ())?;

        conn.execute(
            "ATTACH DATABASE :db_name AS encrypted KEY :key",
            rusqlite::named_params! {
                ":db_name": encrypted_path.to_str(),
                ":key": super::pool::FAKE_KEY,
            },
        )?;

        conn.query_row("SELECT sqlcipher_export('encrypted')", (), |_| Ok(0))?;

        conn.execute("COMMIT", ())?;

        conn.execute("DETACH DATABASE encrypted", ())?;
        conn.close().map_err(|(_, err)| err)?;
    }

    // Swap the databases over
    std::fs::remove_file(path)?;
    std::fs::rename(encrypted_path, path)?;

    Ok(())
}

#[cfg(feature = "test_utils")]
pub fn set_acquire_timeout(timeout_ms: u64) {
    ACQUIRE_TIMEOUT_MS.store(timeout_ms, Ordering::Relaxed);
}
