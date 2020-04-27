//! Functions dealing with obtaining and referencing singleton LMDB environments

use crate::{
    db::DbManager,
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
    static ref ENVIRONMENTS: RwLockSync<HashMap<PathBuf, Environment>> =
        RwLockSync::new(HashMap::new());
    static ref DB_MANAGERS: RwLock<HashMap<PathBuf, Arc<DbManager>>> = RwLock::new(HashMap::new());
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

/// TODO - delete me and use a real keystore when we have one
async fn temp_build_test_keystore() -> KeystoreSender {
    use std::convert::TryFrom;
    let _ = holochain_crypto::crypto_init_sodium();
    let mut keystore = holochain_keystore::test_keystore::spawn_test_keystore(vec![
        holochain_keystore::test_keystore::MockKeypair {
            pub_key: holo_hash::AgentHash::try_from(
                "uhCAkw-zrttiYpdfAYX4fR6W8DPUdheZJ-1QsRA4cTImmzTYUcOr4",
            )
            .unwrap(),
            sec_key: vec![
                220, 218, 15, 212, 178, 51, 204, 96, 121, 97, 6, 205, 179, 84, 80, 159, 84,
                163, 193, 46, 127, 15, 47, 91, 134, 106, 72, 72, 51, 76, 26, 16, 195, 236, 235,
                182, 216, 152, 165, 215, 192, 97, 126, 31, 71, 165, 188, 12, 245, 29, 133, 230,
                73, 251, 84, 44, 68, 14, 28, 76, 137, 166, 205, 54,
            ],
        },
        holochain_keystore::test_keystore::MockKeypair {
            pub_key: holo_hash::AgentHash::try_from(
                "uhCAkomHzekU0-x7p62WmrusdxD2w9wcjdajC88688JGSTEo6cbEK",
            )
            .unwrap(),
            sec_key: vec![
                170, 205, 134, 46, 233, 225, 100, 162, 101, 124, 207, 157, 12, 131, 239, 244,
                216, 190, 244, 161, 209, 56, 159, 135, 240, 134, 88, 28, 48, 75, 227, 244, 162,
                97, 243, 122, 69, 52, 251, 30, 233, 235, 101, 166, 174, 235, 29, 196, 61, 176,
                247, 7, 35, 117, 168, 194, 243, 206, 188, 240, 145, 146, 76, 74,
            ],
        },
    ]).await.unwrap();

    // prepopulate the keystore with our two test agent_hashes
    keystore.generate_sign_keypair_from_pure_entropy().await.unwrap();
    keystore.generate_sign_keypair_from_pure_entropy().await.unwrap();

    keystore
}

/// The canonical representation of a (singleton) LMDB environment.
/// The wrapper contains methods for managing transactions and database connections,
/// tucked away into separate traits.
#[derive(Clone)]
pub struct Environment {
    arc: Arc<RwLock<Rkv>>,
    kind: EnvironmentKind,
    keystore: KeystoreSender,
}

impl Environment {
    /// Create an environment,
    pub async fn new(path_prefix: &Path, kind: EnvironmentKind) -> DatabaseResult<Environment> {
        let mut map = ENVIRONMENTS.write();
        let path = path_prefix.join(kind.path());
        if !path.is_dir() {
            std::fs::create_dir(path.clone())
                .map_err(|_e| DatabaseError::EnvironmentMissing(path.clone()))?;
        }
        let env: Environment = match map.entry(path.clone()) {
            hash_map::Entry::Occupied(e) => e.get().clone(),
            hash_map::Entry::Vacant(e) => e
                .insert({
                    let keystore = temp_build_test_keystore().await;
                    let rkv = rkv_builder(None, None)(&path)?;
                    Environment {
                        arc: Arc::new(RwLock::new(rkv)),
                        kind,
                        keystore,
                    }
                })
                .clone(),
        };
        Ok(env)
    }

    /// Get a reference to the keystore associated with this environment
    pub fn keystore(&self) -> &KeystoreSender {
        &self.keystore
    }

    /// Get a read-only lock on the Environment. The most typical use case is
    /// to get a lock in order to create a read-only transaction. The lock guard
    /// must outlive the transaction, so it has to be returned here and managed
    /// explicitly.
    pub async fn guard(&self) -> EnvironmentRef<'_> {
        EnvironmentRef(self.arc.read().await)
    }

    /// Access the underlying `Rkv` object
    pub async fn inner(&self) -> RwLockReadGuard<'_, Rkv> {
        self.arc.read().await
    }

    /// Accessor for the [EnvironmentKind] of the Environment
    pub fn kind(&self) -> &EnvironmentKind {
        &self.kind
    }

    /// Get access to the singleton database manager ([DbManager]),
    /// in order to access individual LMDB databases
    pub async fn dbs(&self) -> DatabaseResult<Arc<DbManager>> {
        let mut map = DB_MANAGERS.write().await;
        let dbs: Arc<DbManager> = match map.entry(self.inner().await.path().into()) {
            hash_map::Entry::Occupied(e) => e.get().clone(),
            hash_map::Entry::Vacant(e) => e
                .insert(Arc::new(DbManager::new(self.clone()).await?))
                .clone(),
        };
        Ok(dbs)
    }
}

/// The various types of LMDB environment, used to specify the list of databases to initialize in the DbManager
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

/// Newtype wrapper for a read-only lock guard on the Environment
pub struct EnvironmentRef<'e>(RwLockReadGuard<'e, Rkv>);

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

impl<'e> ReadManager<'e> for EnvironmentRef<'e> {
    fn reader(&'e self) -> DatabaseResult<Reader<'e>> {
        let reader = Reader::from(ThreadsafeRkvReader::from(self.0.read()?));
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

impl<'e> WriteManager<'e> for EnvironmentRef<'e> {
    fn writer(&'e self) -> DatabaseResult<Writer<'e>> {
        let writer = Writer::from(self.0.write()?);
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

impl<'e> EnvironmentRef<'e> {
    /// Access the underlying lock guard
    pub fn inner(&'e self) -> &RwLockReadGuard<'e, Rkv> {
        &self.0
    }
}
