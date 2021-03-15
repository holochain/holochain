//! Functions dealing with obtaining and referencing singleton LMDB environments

use crate::{
    conn::{new_connection_pool, PConn, SConn, CONNECTIONS, CONNECTION_POOLS},
    prelude::*,
};
use derive_more::Into;
use futures::Future;
use holochain_keystore::KeystoreSender;
use holochain_zome_types::cell::CellId;
use rusqlite::*;
use shrinkwraprs::Shrinkwrap;
use std::collections::hash_map::Entry;
use std::path::Path;
use std::path::PathBuf;

/// A read-only version of [DbWrite].
/// This environment can only generate read-only transactions, never read-write.
#[derive(Clone)]
pub struct DbRead {
    kind: DbKind,
    path: PathBuf,
    keystore: KeystoreSender,
}

impl DbRead {
    #[deprecated = "rename to `conn`"]
    pub fn guard(&self) -> PConn {
        self.connection_pooled().expect("TODO: Can't fail")
    }

    #[deprecated = "remove this identity function"]
    pub fn inner(&self) -> Self {
        self.clone()
    }

    /// Accessor for the [DbKind] of the DbWrite
    pub fn kind(&self) -> &DbKind {
        &self.kind
    }

    /// Request access to this conductor's keystore
    pub fn keystore(&self) -> &KeystoreSender {
        &self.keystore
    }

    /// The environment's path
    pub fn path(&self) -> &PathBuf {
        &self.path
    }

    #[deprecated = "TODO: use `connection`"]
    fn _connection_naive(&self) -> DatabaseResult<SConn> {
        Ok(SConn::open(&self.path, &self.kind)?)
    }

    #[deprecated = "TODO: use `connection`"]
    fn _connection_singleton(&self) -> DatabaseResult<SConn> {
        let mut map = CONNECTIONS.write();
        let conn = match map.entry(self.path.to_owned()) {
            Entry::Vacant(e) => {
                let conn = SConn::open(&self.path, &self.kind)?;
                e.insert(conn).clone()
            }
            Entry::Occupied(e) => e.get().clone(),
        };

        Ok(conn)
    }

    fn connection_pooled(&self) -> DatabaseResult<PConn> {
        let conn = if let Some(v) = CONNECTION_POOLS.get(&self.path) {
            v.get()?
        } else {
            let pool = new_connection_pool(&self.path, self.kind.clone());
            let mut conn = pool.get()?;
            initialize_database(&mut conn, &self.kind)?;
            CONNECTION_POOLS.insert_new(self.path.clone(), pool);
            conn
        };
        Ok(PConn::new(conn, self.kind.clone()))
    }
}

impl GetTable for DbRead {}
impl GetTable for DbWrite {}

/// The canonical representation of a (singleton) LMDB environment.
/// The wrapper contains methods for managing transactions
/// and database connections,
#[derive(Clone, Shrinkwrap, Into, derive_more::From)]
pub struct DbWrite(DbRead);

impl DbWrite {
    /// Create an environment,
    pub fn new(
        path_prefix: &Path,
        kind: DbKind,
        keystore: KeystoreSender,
    ) -> DatabaseResult<DbWrite> {
        if !path_prefix.is_dir() {
            std::fs::create_dir(path_prefix.clone())
                .map_err(|_e| DatabaseError::EnvironmentMissing(path_prefix.to_owned()))?;
        }
        let path = path_prefix.join(kind.filename());
        let env: DbWrite = DbWrite(DbRead {
            kind,
            keystore,
            path,
        });
        Ok(env)
    }

    /// Create a Cell environment (slight shorthand)
    pub fn new_cell(
        path_prefix: &Path,
        cell_id: CellId,
        keystore: KeystoreSender,
    ) -> DatabaseResult<Self> {
        Self::new(path_prefix, DbKind::Cell(cell_id), keystore)
    }

    #[deprecated = "remove this identity function"]
    pub fn guard(&self) -> PConn {
        self.0.guard()
    }

    /// Remove the db and directory
    pub async fn remove(self) -> DatabaseResult<()> {
        std::fs::remove_dir_all(&self.0.path)?;
        Ok(())
    }
}

/// The various types of LMDB environment, used to specify the list of databases to initialize
#[derive(Clone, Debug, derive_more::Display)]
pub enum DbKind {
    /// Specifies the environment used by each Cell
    Cell(CellId),
    /// Specifies the environment used by a Conductor
    Conductor,
    /// Specifies the environment used to save wasm
    Wasm,
    /// State of the p2p network
    P2p,
}

impl DbKind {
    /// Constuct a partial Path based on the kind
    fn filename(&self) -> PathBuf {
        let mut path = match self {
            DbKind::Cell(cell_id) => PathBuf::from(cell_id.to_string()),
            DbKind::Conductor => PathBuf::from("conductor"),
            DbKind::Wasm => PathBuf::from("wasm"),
            DbKind::P2p => PathBuf::from("p2p"),
        };
        path.set_extension("sqlite3");
        path
    }
}

/// Implementors are able to create a new read-only LMDB transaction
pub trait ReadManager<'e> {
    /// Run a closure, passing in a new read-only transaction
    fn with_reader<E, R, F>(&'e mut self, f: F) -> Result<R, E>
    where
        E: From<DatabaseError>,
        F: 'e + FnOnce(Reader) -> Result<R, E>;

    #[cfg(feature = "test_utils")]
    /// Same as with_reader, but with no Results: everything gets unwrapped
    fn with_reader_test<R, F>(&'e mut self, f: F) -> R
    where
        F: 'e + FnOnce(Reader) -> R;
}

/// Implementors are able to create a new read-write LMDB transaction
pub trait WriteManager<'e> {
    /// Run a closure, passing in a mutable reference to a read-write
    /// transaction, and commit the transaction after the closure has run.
    /// If there is a LMDB error, recover from it and re-run the closure.
    // FIXME: B-01566: implement write failure detection
    fn with_commit<E, R, F>(&'e mut self, f: F) -> Result<R, E>
    where
        E: From<DatabaseError>,
        F: 'e + FnOnce(&mut Writer) -> Result<R, E>;

    // /// Get a raw read-write transaction for this environment.
    // /// It is preferable to use WriterManager::with_commit for database writes,
    // /// which can properly recover from and manage write failures
    // fn writer_unmanaged(&'e mut self) -> DatabaseResult<Writer<'e>>;
}

impl<'e> ReadManager<'e> for SConn {
    fn with_reader<E, R, F>(&'e mut self, f: F) -> Result<R, E>
    where
        E: From<DatabaseError>,
        F: 'e + FnOnce(Reader) -> Result<R, E>,
    {
        let mut g = self.inner();
        let txn = g.transaction().map_err(DatabaseError::from)?;
        let reader = Reader::from(txn);
        f(reader)
    }

    #[cfg(feature = "test_utils")]
    fn with_reader_test<R, F>(&'e mut self, f: F) -> R
    where
        F: 'e + FnOnce(Reader) -> R,
    {
        self.with_reader(|r| DatabaseResult::Ok(f(r))).unwrap()
    }
}

impl<'e> WriteManager<'e> for SConn {
    fn with_commit<E, R, F>(&'e mut self, f: F) -> Result<R, E>
    where
        E: From<DatabaseError>,
        F: 'e + FnOnce(&mut Writer) -> Result<R, E>,
    {
        let mut b = self.inner();
        let txn = b.transaction().map_err(DatabaseError::from)?;
        let mut writer = Writer::from(txn);
        let result = f(&mut writer)?;
        writer.commit().map_err(DatabaseError::from)?;
        Ok(result)
    }
}

impl<'e> ReadManager<'e> for PConn {
    fn with_reader<E, R, F>(&'e mut self, f: F) -> Result<R, E>
    where
        E: From<DatabaseError>,
        F: 'e + FnOnce(Reader) -> Result<R, E>,
    {
        let txn = self.transaction().map_err(DatabaseError::from)?;
        let reader = Reader::from(txn);
        f(reader)
    }

    #[cfg(feature = "test_utils")]
    fn with_reader_test<R, F>(&'e mut self, f: F) -> R
    where
        F: 'e + FnOnce(Reader) -> R,
    {
        self.with_reader(|r| DatabaseResult::Ok(f(r))).unwrap()
    }
}

impl<'e> WriteManager<'e> for PConn {
    fn with_commit<E, R, F>(&'e mut self, f: F) -> Result<R, E>
    where
        E: From<DatabaseError>,
        F: 'e + FnOnce(&mut Writer) -> Result<R, E>,
    {
        let txn = self.transaction().map_err(DatabaseError::from)?;
        let mut writer = Writer::from(txn);
        let result = f(&mut writer)?;
        writer.commit().map_err(DatabaseError::from)?;
        Ok(result)
    }
}

#[derive(Debug)]
pub struct OptimisticRetryError<E: std::error::Error>(Vec<E>);

pub async fn optimistic_retry_async<Func, Fut, T, E>(
    ctx: &str,
    mut f: Func,
) -> Result<T, OptimisticRetryError<E>>
where
    Func: FnMut() -> Fut,
    Fut: Future<Output = Result<T, E>>,
    E: std::error::Error,
{
    const NUM_CONSECUTIVE_FAILURES: usize = 10;
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
    }
}
