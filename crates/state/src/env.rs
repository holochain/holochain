use crate::{
    db::DbManager,
    error::{DatabaseError, DatabaseResult},
    transaction::{Reader, Writer},
};
use lazy_static::lazy_static;
use parking_lot::RwLock;
use rkv::{EnvironmentFlags, Rkv};
use std::{
    collections::{hash_map, HashMap},
    path::{Path, PathBuf},
    sync::Arc,
};

const DEFAULT_INITIAL_MAP_SIZE: usize = 100 * 1024 * 1024;
const MAX_DBS: u32 = 32;

lazy_static! {
    static ref ENVIRONMENTS: RwLock<HashMap<PathBuf, Environment>> = RwLock::new(HashMap::new());
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

/// A standard way to create a representation of an LMDB environment suitable for Holochain
pub fn create_lmdb_env(path: &Path) -> DatabaseResult<Environment> {
    let mut map = ENVIRONMENTS.write();
    let env: Environment = match map.entry(path.into()) {
        hash_map::Entry::Occupied(e) => e.get().clone(),
        hash_map::Entry::Vacant(e) => e
            .insert({
                let rkv = rkv_builder(None, None)(path)?;
                Environment(Arc::new(rkv))
            })
            .clone(),
    };
    Ok(env)
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

/// The canonical representation of a reference to a (singleton) LMDB environment.
/// The wrapper contains methods for managing transactions and database connections,
/// tucked away into separate traits.
#[derive(Clone)]
pub struct Environment(Arc<Rkv>);

pub trait ReadManager {
    fn reader(&self) -> DatabaseResult<Reader>;

    /// Make chnage to database reader
    fn with_reader<E, R, F>(&self, f: F) -> Result<R, E>
    where
        E: From<DatabaseError>,
        F: FnOnce(Reader) -> Result<R, E>;
}

pub trait WriteManager {
    fn writer(&self) -> DatabaseResult<Writer>;

    fn with_commit<E, R, F>(&self, f: F) -> Result<R, E>
    where
        E: From<DatabaseError>,
        F: FnOnce(&mut Writer) -> Result<R, E>;
}

impl ReadManager for Environment {
    fn reader(&self) -> DatabaseResult<Reader> {
        Ok(Reader::from(self.0.read()?))
    }

    fn with_reader<E, R, F>(&self, f: F) -> Result<R, E>
    where
        E: From<DatabaseError>,
        F: FnOnce(Reader) -> Result<R, E>,
    {
        f(Reader::from(self.0.read().map_err(Into::into)?))
    }
}

impl WriteManager for Environment {
    fn writer(&self) -> DatabaseResult<Writer> {
        Ok(self.0.write()?.into())
    }

    fn with_commit<E, R, F>(&self, f: F) -> Result<R, E>
    where
        E: From<DatabaseError>,
        F: FnOnce(&mut Writer) -> Result<R, E>,
    {
        let mut writer = self.0.write().map_err(Into::into)?.into();
        let result = f(&mut writer);
        writer.commit().map_err(Into::into)?;
        result
    }
}

impl Environment {
    pub fn inner(&self) -> &Rkv {
        &self.0
    }

    pub fn dbs(&self) -> DatabaseResult<Arc<DbManager>> {
        let mut map = DB_MANAGERS.write();
        let dbs = map
            .entry(self.0.as_ref().path().into())
            .or_insert_with(|| Arc::new(DbManager::new(self.clone()).expect("TODO")));
        Ok(dbs.clone())
        // DbManager::new(self.env())
    }
}
