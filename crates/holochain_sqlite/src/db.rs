//! Functions dealing with obtaining and referencing singleton databases

use crate::{
    conn::{new_connection_pool, ConnectionPool, PConn, DATABASE_HANDLES},
    prelude::*,
};
use derive_more::Into;
use futures::Future;
use holo_hash::DnaHash;
use holochain_zome_types::cell::CellId;
use kitsune_p2p::KitsuneSpace;
use rusqlite::*;
use shrinkwraprs::Shrinkwrap;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

mod p2p;
pub use p2p::*;

/// A read-only version of [DbWrite].
/// This environment can only generate read-only transactions, never read-write.
#[derive(Clone)]
pub struct DbRead {
    kind: DbKind,
    path: PathBuf,
    connection_pool: ConnectionPool,
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
        let path = path_prefix.join(kind.filename());
        if let Some(v) = DATABASE_HANDLES.get(&path) {
            Ok(v.clone())
        } else {
            let db = Self::new(path_prefix, kind)?;
            DATABASE_HANDLES.insert_new(path, db.clone());
            Ok(db)
        }
    }

    pub(crate) fn new(path_prefix: &Path, kind: DbKind) -> DatabaseResult<Self> {
        let path = path_prefix.join(kind.filename());
        let parent = path
            .parent()
            .ok_or_else(|| DatabaseError::DatabaseMissing(path_prefix.to_owned()))?;
        if !parent.is_dir() {
            std::fs::create_dir_all(parent)
                .map_err(|_e| DatabaseError::DatabaseMissing(parent.to_owned()))?;
        }
        let pool = new_connection_pool(&path, kind.clone());
        let mut conn = pool.get()?;
        crate::table::initialize_database(&mut conn, &kind)?;

        Ok(DbWrite(DbRead {
            kind,
            path,
            connection_pool: pool,
        }))
    }

    /// Create a unique db in a temp dir with no static management of the
    /// connection pool, useful for testing.
    #[cfg(any(test, feature = "test_utils"))]
    pub fn test(tmpdir: &tempdir::TempDir, kind: DbKind) -> DatabaseResult<Self> {
        Self::new(tmpdir.path(), kind)
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

/// The various types of database, used to specify the list of databases to initialize
#[derive(Clone, Debug, derive_more::Display)]
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
    P2p(Arc<KitsuneSpace>),
}

impl DbKind {
    /// Constuct a partial Path based on the kind
    fn filename(&self) -> PathBuf {
        let mut path: PathBuf = match self {
            DbKind::Cell(cell_id) => ["cell", &cell_id.to_string()].iter().collect(),
            DbKind::Cache(dna) => ["cache", &format!("cache-{}", dna)].iter().collect(),
            DbKind::Conductor => ["conductor", "conductor"].iter().collect(),
            DbKind::Wasm => ["wasm", "wasm"].iter().collect(),
            DbKind::P2p(space) => ["p2p", &format!("p2p-{}", space)].iter().collect(),
        };
        path.set_extension("sqlite3");
        path
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
    fn with_commit<E, R, F>(&'e mut self, f: F) -> Result<R, E>
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
        self.with_commit(|w| DatabaseResult::Ok(f(w)))
    }
}

impl<'e> ReadManager<'e> for PConn {
    fn with_reader<E, R, F>(&'e mut self, f: F) -> Result<R, E>
    where
        E: From<DatabaseError>,
        F: 'e + FnOnce(Transaction) -> Result<R, E>,
    {
        thread_local!(static LAST_T: std::cell::RefCell<String> = std::cell::RefCell::new("".to_string()));
        let t_now = std::time::Instant::now();
        // self.trace(Some(|s| {
        //     LAST_T.with(|f| {
        //         *f.borrow_mut() = s.to_string();
        //     });
        // }));
        let txn = self.transaction().map_err(DatabaseError::from)?;
        let now = std::time::Instant::now();
        let r = f(txn);
        let dur = now.elapsed();
        if dur.as_millis()
            > std::env::var_os("CONN_PROFILE_MS")
                .and_then(|s| s.to_string_lossy().parse::<u128>().ok())
                .unwrap_or(20)
        {
            let s = tracing::debug_span!("conn_trace", ?dur, t_time = ?t_now.elapsed());
            let _g = s.enter();
            LAST_T.with(|f| {
                let stmt = f.borrow_mut();
                let len = std::cmp::min(stmt.len(), 200);
                tracing::debug!(read_statement = %stmt[..len]);
                tracing::trace!(%stmt);
            });
        }
        r
    }

    #[cfg(feature = "test_utils")]
    fn with_reader_test<R, F>(&'e mut self, f: F) -> R
    where
        F: 'e + FnOnce(Transaction) -> R,
    {
        self.with_reader(|r| DatabaseResult::Ok(f(r))).unwrap()
    }
}

impl<'e> WriteManager<'e> for PConn {
    fn with_commit<E, R, F>(&'e mut self, f: F) -> Result<R, E>
    where
        E: From<DatabaseError>,
        F: 'e + FnOnce(&mut Transaction) -> Result<R, E>,
    {
        thread_local!(static LAST_T: std::cell::RefCell<String> = std::cell::RefCell::new("".to_string()));
        let t_now = std::time::Instant::now();
        // self.trace(Some(|s| {
        //     LAST_T.with(|f| {
        //         *f.borrow_mut() = s.to_string();
        //     });
        // }));
        let mut txn = self
            .transaction_with_behavior(TransactionBehavior::Exclusive)
            .map_err(DatabaseError::from)?;
        let now = std::time::Instant::now();
        let result = f(&mut txn)?;

        let dur = now.elapsed();
        if dur.as_millis()
            > std::env::var_os("CONN_PROFILE_MS")
                .and_then(|s| s.to_string_lossy().parse::<u128>().ok())
                .unwrap_or(20)
        {
            let s = tracing::debug_span!("conn_trace", ?dur, t_time = ?t_now.elapsed());
            let _g = s.enter();
            LAST_T.with(|f| {
                let stmt = f.borrow_mut();
                let len = std::cmp::min(stmt.len(), 200);
                tracing::debug!(write_statement = %stmt[..len]);
                tracing::trace!(%stmt);
            });
        }
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
