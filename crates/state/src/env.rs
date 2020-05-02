//! Functions dealing with obtaining and referencing singleton LMDB environments

use crate::{
    db::{get_db, initialize_databases, DbKey, GetDb},
    error::{DatabaseError, DatabaseResult},
    transaction::{Reader, ThreadsafeRkvReader, Writer},
};
use holochain_keystore::KeystoreSender;
use holochain_types::cell::CellId;
use lazy_static::lazy_static;
use parking_lot::RwLock as RwLockSync;
use rkv::{EnvironmentFlags, Rkv};
use std::{
    collections::{hash_map, HashMap},
    path::{Path, PathBuf},
    sync::Arc,
};
use tokio::sync::{RwLock, RwLockReadGuard};

const DEFAULT_INITIAL_MAP_SIZE: usize = 100 * 1024 * 1024;
const MAX_DBS: u32 = 32;

lazy_static! {
    static ref ENVIRONMENTS: RwLockSync<HashMap<PathBuf, EnvironmentRw>> =
        RwLockSync::new(HashMap::new());
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

/// The canonical representation of a (singleton) LMDB environment.
/// The wrapper contains methods for managing transactions and database connections,
/// tucked away into separate traits.
#[derive(Clone)]
pub struct EnvironmentRw {
    arc: Arc<RwLock<Rkv>>,
    kind: EnvironmentKind,
    path: PathBuf,
    keystore: KeystoreSender,
}

impl EnvironmentRw {
    /// Create an environment,
    pub async fn new(
        path_prefix: &Path,
        kind: EnvironmentKind,
        keystore: KeystoreSender,
    ) -> DatabaseResult<EnvironmentRw> {
        let mut map = ENVIRONMENTS.write();
        let path = path_prefix.join(kind.path());
        if !path.is_dir() {
            std::fs::create_dir(path.clone())
                .map_err(|_e| DatabaseError::EnvironmentMissing(path.clone()))?;
        }
        let env: EnvironmentRw = match map.entry(path.clone()) {
            hash_map::Entry::Occupied(e) => e.get().clone(),
            hash_map::Entry::Vacant(e) => e
                .insert({
                    let rkv = rkv_builder(None, None)(&path)?;
                    initialize_databases(&rkv, &kind).await?;
                    EnvironmentRw {
                        arc: Arc::new(RwLock::new(rkv)),
                        kind,
                        keystore,
                        path,
                    }
                })
                .clone(),
        };
        Ok(env)
    }

    /// Get a read-only lock on the EnvironmentRw. The most typical use case is
    /// to get a lock in order to create a read-only transaction. The lock guard
    /// must outlive the transaction, so it has to be returned here and managed
    /// explicitly.
    pub async fn guard<'e>(&'e self) -> EnvironmentRefRw<'e> {
        EnvironmentRefRw {
            rkv: self.arc.read().await,
            keystore: self.keystore.clone(),
        }
    }

    /// Access the underlying `Rkv` object
    pub async fn inner(&self) -> RwLockReadGuard<'_, Rkv> {
        self.arc.read().await
    }

    /// Accessor for the [EnvironmentKind] of the EnvironmentRw
    pub fn kind(&self) -> &EnvironmentKind {
        &self.kind
    }

    /// Request access to this conductor's keystore
    pub fn keystore(&self) -> &KeystoreSender {
        &self.keystore
    }

    /// Return a `GetDb`, which can synchronously get databases from this environment
    pub async fn dbs<'e>(&'e self) -> impl GetDb + 'e {
        self.guard().await
    }
}

impl GetDb for EnvironmentRw {
    fn get_db<V: 'static + Copy + Send + Sync>(&self, key: &'static DbKey<V>) -> DatabaseResult<V> {
        get_db(&self.path, key)
    }

    fn keystore(&self) -> KeystoreSender {
        self.keystore.clone()
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
}

impl EnvironmentKind {
    /// Constuct a partial Path based on the kind
    fn path(&self) -> PathBuf {
        match self {
            EnvironmentKind::Cell(cell_id) => PathBuf::from(cell_id.to_string()),
            EnvironmentKind::Conductor => PathBuf::from("conductor"),
            EnvironmentKind::Wasm => PathBuf::from("wasm"),
        }
    }
}

/// Newtype wrapper for a read-only lock guard on the EnvironmentRw
pub struct EnvironmentRefRw<'e> {
    rkv: RwLockReadGuard<'e, Rkv>,
    keystore: KeystoreSender,
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
    /// Create a new read-write LMDB transaction
    fn writer(&'e self) -> DatabaseResult<Writer<'e>>;

    /// Run a closure, passing in a mutable reference to a read-write
    /// transaction, and commit the transaction after the closure has run
    fn with_commit<E, R, F: Send>(&self, f: F) -> Result<R, E>
    where
        E: From<DatabaseError>,
        F: FnOnce(&mut Writer) -> Result<R, E>;
}

impl<'e> ReadManager<'e> for EnvironmentRefRw<'e> {
    fn reader(&'e self) -> DatabaseResult<Reader<'e>> {
        let reader = Reader::from(ThreadsafeRkvReader::from(self.rkv.read()?));
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

impl<'e> WriteManager<'e> for EnvironmentRefRw<'e> {
    fn writer(&'e self) -> DatabaseResult<Writer<'e>> {
        let writer = Writer::from(self.rkv.write()?);
        Ok(writer)
    }

    fn with_commit<E, R, F: Send>(&self, f: F) -> Result<R, E>
    where
        E: From<DatabaseError>,
        F: FnOnce(&mut Writer) -> Result<R, E>,
    {
        let mut writer = self.writer()?;
        let result = f(&mut writer);
        writer.commit().map_err(Into::into)?;
        result
    }
}

impl<'e> GetDb for EnvironmentRefRw<'e> {
    fn get_db<V: 'static + Copy + Send + Sync>(&self, key: &'static DbKey<V>) -> DatabaseResult<V> {
        let path = self.inner().path().clone();
        get_db(path, key)
    }

    fn keystore(&self) -> KeystoreSender {
        self.keystore()
    }
}

impl<'e> EnvironmentRefRw<'e> {
    /// Access the underlying lock guard
    pub(crate) fn inner(&'e self) -> &RwLockReadGuard<'e, Rkv> {
        &self.rkv
    }

    pub(crate) fn keystore(&self) -> KeystoreSender {
        self.keystore.clone()
    }
}

