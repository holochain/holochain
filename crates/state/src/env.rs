use std::path::PathBuf;
use crate::{
    db::DbManager,
    error::{WorkspaceError, WorkspaceResult},
    exports::Writer,
    reader::Reader,
};
use lazy_static::lazy_static;
use rkv::{EnvironmentFlags, Rkv};
use std::{collections::HashMap, path::Path, sync::Arc};

const DEFAULT_INITIAL_MAP_SIZE: usize = 100 * 1024 * 1024;
const MAX_DBS: u32 = 32;

// lazy_static! {
//     static ref ENVIRONMENTS: HashMap<PathBuf, EnvArc> = HashMap::new();
//     static ref DB_MANAGERS: HashMap<PathBuf, DbManager> = HashMap::new();
// }

#[cfg(feature = "lmdb_no_tls")]
fn default_flags() -> EnvironmentFlags {
    EnvironmentFlags::WRITE_MAP | EnvironmentFlags::MAP_ASYNC | EnvironmentFlags::NO_TLS
}

#[cfg(not(feature = "lmdb_no_tls"))]
fn default_flags() -> EnvironmentFlags {
    EnvironmentFlags::WRITE_MAP | EnvironmentFlags::MAP_ASYNC
}

/// A standard way to create a representation of an LMDB environment suitable for Holochain
/// TODO: put this behind a singleton HashMap, just like rkv::Manager,
///     but wrap it in Arc<_> instead of Arc<RwLock<_>>
pub fn create_lmdb_env(path: &Path) -> WorkspaceResult<EnvArc> {
    let initial_map_size = None;
    let flags = None;
    // let rkv = Manager::singleton()
    //     .write()
    //     .unwrap()
    //     .get_or_create(path, rkv_builder(initial_map_size, flags))
    //     .unwrap();
    let rkv = rkv_builder(initial_map_size, flags)(path)?;
    Ok(EnvArc::new(rkv))
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
            // The flags WRITE_MAP and MAP_ASYNC make writes waaaaay faster by async writing to disk rather than blocking
            // There is some loss of data integrity guarantees that comes with this.
            // NO_TLS associates read slots with the transaction object instead of the thread, which is crucial for us
            // so we can have multiple read transactions per thread (since futures can run on any thread)
            .set_flags(flags.unwrap_or_else(default_flags));
        Rkv::from_env(path, env_builder)
    }
}

/// There can only be one owned value of `Rkv`. EnvArc is a simple wrapper around an `Arc<Rkv>`,
/// which can produce as many `Env` values as needed.
#[derive(Clone)]
pub struct EnvArc {
    rkv: Arc<Rkv>,
}

impl EnvArc {
    fn new(rkv: Rkv) -> Self {
        Self { rkv: Arc::new(rkv) }
    }

    pub fn env(&self) -> Env {
        Env(&self.rkv)
    }

    // TODO: make sure this is never called more than once per environment!
    pub fn dbs(&self) -> WorkspaceResult<DbManager> {
        DbManager::new(self.env())
    }
}

/// The canonical representation of a reference to a (singleton) LMDB environment.
/// These are produced by an `EnvArc`.
/// The wrapper contains methods for managing transactions and database connections,
/// tucked away into separate traits.
pub struct Env<'e>(&'e Rkv);

pub trait ReadManager {
    fn reader(&self) -> WorkspaceResult<Reader>;

    fn with_reader<E, R, F>(&self, f: F) -> Result<R, E>
    where
        E: From<WorkspaceError>,
        F: FnOnce(Reader) -> Result<R, E>;
}

pub trait WriteManager {
    fn writer(&self) -> WorkspaceResult<Writer>;

    fn with_commit<E, R, F>(&self, f: F) -> Result<R, E>
    where
        E: From<WorkspaceError>,
        F: FnOnce(&mut Writer) -> Result<R, E>;
}

impl<'e> ReadManager for Env<'e> {
    fn reader(&self) -> WorkspaceResult<Reader> {
        Ok(Reader::new(self.0.read()?))
    }

    fn with_reader<E, R, F>(&self, f: F) -> Result<R, E>
    where
        E: From<WorkspaceError>,
        F: FnOnce(Reader) -> Result<R, E>,
    {
        f(Reader::new(self.0.read().map_err(Into::into)?))
    }
}

impl<'e> WriteManager for Env<'e> {
    fn writer(&self) -> WorkspaceResult<Writer> {
        Ok(self.0.write()?)
    }

    fn with_commit<E, R, F>(&self, f: F) -> Result<R, E>
    where
        E: From<WorkspaceError>,
        F: FnOnce(&mut Writer) -> Result<R, E>,
    {
        let mut writer = self.0.write().map_err(Into::into)?;
        let result = f(&mut writer);
        writer.commit().map_err(Into::into)?;
        result
    }
}

impl<'e> Env<'e> {
    pub fn inner(&self) -> &Rkv {
        &self.0
    }
}
