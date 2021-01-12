//! Functions dealing with obtaining and referencing singleton LMDB environments

use crate::db::initialize_databases;
use crate::error::DatabaseError;
use crate::error::DatabaseResult;
use crate::transaction::Reader;
use crate::transaction::Writer;
use derive_more::Into;
use holochain_keystore::KeystoreSender;
use holochain_zome_types::cell::CellId;
use lazy_static::lazy_static;
use parking_lot::RwLock;
use parking_lot::RwLockReadGuard;
use rkv::EnvironmentFlags;
use rkv::Rkv;
use shrinkwraprs::Shrinkwrap;
use std::collections::hash_map;
use std::collections::HashMap;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

const DEFAULT_INITIAL_MAP_SIZE: usize = 100 * 1024 * 1024; // 100MB
const MAX_DBS: u32 = 32;

lazy_static! {
    static ref ENVIRONMENTS: RwLock<HashMap<PathBuf, EnvironmentWrite>> = {
        // This is just a convenient place that we know gets initialized
        // both in the final binary holochain && in all relevant tests
        //
        // Holochain (and most binaries) are left in invalid states
        // if a thread panic!s - switch to failing fast in that case.
        //
        // We tried putting `panic = "abort"` in the Cargo.toml,
        // but somehow that breaks the wasmer / test_utils integration.

        let orig_handler = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |panic_info| {
            // print the panic message
            eprintln!("FATAL PANIC {:#?}", panic_info);
            // invoke the original handler
            orig_handler(panic_info);
            // // Abort the process
            // // TODO - we need a better solution than this, but if there is
            // // no better solution, we can uncomment the following line:
            // std::process::abort();
        }));

        RwLock::new(HashMap::new())
    };
}

fn default_flags() -> EnvironmentFlags {
    // The flags WRITE_MAP and MAP_ASYNC make writes waaaaay faster by async writing to disk rather than blocking
    // There is some loss of data integrity guarantees that comes with this.
    EnvironmentFlags::WRITE_MAP | EnvironmentFlags::MAP_ASYNC
}

#[cfg(feature = "lmdb_no_tls")]
fn required_flags() -> EnvironmentFlags {
    // NO_TLS associates read slots with the transaction object instead of the thread, which is crucial for us
    // so we can have multiple read transactions per thread (since futures can run on any thread)
    EnvironmentFlags::NO_TLS
}

#[cfg(not(feature = "lmdb_no_tls"))]
fn required_flags() -> EnvironmentFlags {
    EnvironmentFlags::default()
}

fn rkv_builder(
    initial_map_size: Option<usize>,
    flags: Option<EnvironmentFlags>,
) -> impl (Fn(&Path) -> Result<Rkv, rkv::StoreError>) {
    move |path: &Path| {
        let mut env_builder = Rkv::environment_builder();
        env_builder
            // max size of memory map, can be changed later
            .set_map_size(initial_map_size.unwrap_or(DEFAULT_INITIAL_MAP_SIZE))
            // max number of DBs in this environment
            .set_max_dbs(MAX_DBS)
            .set_flags(flags.unwrap_or_else(default_flags) | required_flags());
        Rkv::from_env(path, env_builder)
    }
}

/// A read-only version of [EnvironmentWrite].
/// This environment can only generate read-only transactions, never read-write.
#[derive(Clone)]
pub struct EnvironmentRead {
    arc: Arc<RwLock<Rkv>>,
    kind: EnvironmentKind,
    path: PathBuf,
    keystore: KeystoreSender,
}

impl EnvironmentRead {
    /// Get a read-only lock on the EnvironmentWrite. The most typical use case is
    /// to get a lock in order to create a read-only transaction. The lock guard
    /// must outlive the transaction, so it has to be returned here and managed
    /// explicitly.
    pub fn guard(&self) -> EnvironmentReadRef<'_> {
        EnvironmentReadRef {
            rkv: self.arc.read(),
        }
    }

    /// Accessor for the [EnvironmentKind] of the EnvironmentWrite
    pub fn kind(&self) -> &EnvironmentKind {
        &self.kind
    }

    /// Request access to this conductor's keystore
    pub fn keystore(&self) -> &KeystoreSender {
        &self.keystore
    }

    /// The environments path
    pub fn path(&self) -> &PathBuf {
        &self.path
    }
}

/// The canonical representation of a (singleton) LMDB environment.
/// The wrapper contains methods for managing transactions
/// and database connections,
#[derive(Clone, Shrinkwrap, Into, derive_more::From)]
pub struct EnvironmentWrite(EnvironmentRead);

impl EnvironmentWrite {
    /// Create an environment,
    pub fn new(
        path_prefix: &Path,
        kind: EnvironmentKind,
        keystore: KeystoreSender,
    ) -> DatabaseResult<EnvironmentWrite> {
        let mut map = ENVIRONMENTS.write();
        let path = path_prefix.join(kind.path());
        if !path.is_dir() {
            std::fs::create_dir(path.clone())
                .map_err(|_e| DatabaseError::EnvironmentMissing(path.clone()))?;
        }
        let env: EnvironmentWrite = match map.entry(path.clone()) {
            hash_map::Entry::Occupied(e) => e.get().clone(),
            hash_map::Entry::Vacant(e) => e
                .insert({
                    let rkv = rkv_builder(None, None)(&path)?;
                    tracing::debug!("Initializing databases for path {:?}", path);
                    initialize_databases(&rkv, &kind)?;
                    EnvironmentWrite(EnvironmentRead {
                        arc: Arc::new(RwLock::new(rkv)),
                        kind,
                        keystore,
                        path,
                    })
                })
                .clone(),
        };
        Ok(env)
    }

    /// Create a Cell environment (slight shorthand)
    pub fn new_cell(
        path_prefix: &Path,
        cell_id: CellId,
        keystore: KeystoreSender,
    ) -> DatabaseResult<Self> {
        Self::new(path_prefix, EnvironmentKind::Cell(cell_id), keystore)
    }

    /// Get a read-only lock guard on the environment.
    /// This reference can create read-write transactions.
    pub fn guard(&self) -> EnvironmentWriteRef<'_> {
        EnvironmentWriteRef(self.0.guard())
    }

    /// Remove the db and directory
    pub async fn remove(self) -> DatabaseResult<()> {
        let mut map = ENVIRONMENTS.write();
        map.remove(&self.0.path);
        // TODO remove this db from the DB_MAP_MAP?
        // remove the directory
        std::fs::remove_dir_all(&self.0.path)?;
        Ok(())
    }
}

/// The various types of LMDB environment, used to specify the list of databases to initialize
#[derive(Clone)]
pub enum EnvironmentKind {
    /// Specifies the environment used by each Cell
    Cell(CellId),
    /// Specifies the environment used by a Conductor
    Conductor,
    /// Specifies the environment used to save wasm
    Wasm,
    /// State of the p2p network
    P2p,
}

impl EnvironmentKind {
    /// Constuct a partial Path based on the kind
    fn path(&self) -> PathBuf {
        match self {
            EnvironmentKind::Cell(cell_id) => PathBuf::from(cell_id.to_string()),
            EnvironmentKind::Conductor => PathBuf::from("conductor"),
            EnvironmentKind::Wasm => PathBuf::from("wasm"),
            EnvironmentKind::P2p => PathBuf::from("p2p"),
        }
    }
}
/// A reference to a read-only EnvironmentRead.
/// This has the distinction of being unable to create a read-write transaction,
/// because unlike [EnvironmentWriteRef], this does not implement WriteManager
pub struct EnvironmentReadRef<'e> {
    rkv: RwLockReadGuard<'e, Rkv>,
}

impl<'e> EnvironmentReadRef<'e> {
    /// Access the wrapped Rkv
    pub fn rkv(&self) -> &Rkv {
        &self.rkv
    }
}

/// Implementors are able to create a new read-only LMDB transaction
pub trait ReadManager<'e> {
    /// Create a new read-only LMDB transaction
    fn reader(&'e self) -> DatabaseResult<Reader<'e>>;

    /// Run a closure, passing in a new read-only transaction
    fn with_reader<E, R, F: Send>(&self, f: F) -> Result<R, E>
    where
        E: From<DatabaseError>,
        F: FnOnce(Reader) -> Result<R, E>;
}

/// Implementors are able to create a new read-write LMDB transaction
pub trait WriteManager<'e> {
    /// Run a closure, passing in a mutable reference to a read-write
    /// transaction, and commit the transaction after the closure has run.
    /// If there is a LMDB error, recover from it and re-run the closure.
    // FIXME: B-01566: implement write failure detection
    fn with_commit<E, R, F: Send>(&self, f: F) -> Result<R, E>
    where
        E: From<DatabaseError>,
        F: FnOnce(&mut Writer) -> Result<R, E>;
}

impl<'e> ReadManager<'e> for EnvironmentReadRef<'e> {
    fn reader(&'e self) -> DatabaseResult<Reader<'e>> {
        let reader = Reader::from(self.rkv.read()?);
        Ok(reader)
    }

    fn with_reader<E, R, F: Send>(&self, f: F) -> Result<R, E>
    where
        E: From<DatabaseError>,
        F: FnOnce(Reader) -> Result<R, E>,
    {
        f(self.reader()?)
    }
}

impl<'e> WriteManager<'e> for EnvironmentWriteRef<'e> {
    fn with_commit<E, R, F: Send>(&self, f: F) -> Result<R, E>
    where
        E: From<DatabaseError>,
        F: FnOnce(&mut Writer) -> Result<R, E>,
    {
        let mut writer = Writer::from(self.rkv.write().map_err(Into::into)?);
        let result = f(&mut writer)?;
        writer.commit().map_err(Into::into)?;
        Ok(result)
    }
}

impl<'e> WriteManager<'e> for EnvironmentWrite {
    fn with_commit<E, R, F: Send>(&self, f: F) -> Result<R, E>
    where
        E: From<DatabaseError>,
        F: FnOnce(&mut Writer) -> Result<R, E>,
    {
        EnvironmentWriteRef::with_commit(&self.guard(), f)
    }
}

impl<'e> EnvironmentWriteRef<'e> {
    /// Access the underlying Rkv lock guard
    #[cfg(test)]
    pub(crate) fn inner(&'e self) -> &Rkv {
        &self.rkv
    }

    /// Get a raw read-write transaction for this environment.
    /// It is preferable to use WriterManager::with_commit for database writes,
    /// which can properly recover from and manage write failures
    pub fn writer_unmanaged(&'e self) -> DatabaseResult<Writer<'e>> {
        let writer = Writer::from(self.rkv.write()?);
        Ok(writer)
    }
}

/// A reference to a EnvironmentWrite
#[derive(Shrinkwrap, Into)]
pub struct EnvironmentWriteRef<'e>(EnvironmentReadRef<'e>);

impl<'e> ReadManager<'e> for EnvironmentWriteRef<'e> {
    fn reader(&'e self) -> DatabaseResult<Reader<'e>> {
        self.0.reader()
    }

    fn with_reader<E, R, F: Send>(&self, f: F) -> Result<R, E>
    where
        E: From<DatabaseError>,
        F: FnOnce(Reader) -> Result<R, E>,
    {
        self.0.with_reader(f)
    }
}
