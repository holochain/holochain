use crate::{
    db::DbManager,
    error::{DatabaseError, DatabaseResult},
    transaction::{Reader, ThreadsafeRkvReader, Writer},
};
use async_trait::async_trait;
use lazy_static::lazy_static;
use parking_lot::RwLock as RwLockSync;
use rkv::{EnvironmentFlags, Rkv};
use shrinkwraprs::Shrinkwrap;
use std::{
    collections::{hash_map, HashMap},
    path::{Path, PathBuf},
    sync::Arc,
};
use tokio::sync::{RwLock, RwLockReadGuard, RwLockWriteGuard};

const DEFAULT_INITIAL_MAP_SIZE: usize = 100 * 1024 * 1024;
const MAX_DBS: u32 = 32;

lazy_static! {
    static ref ENVIRONMENTS: RwLockSync<HashMap<PathBuf, Environment>> =
        RwLockSync::new(HashMap::new());
    static ref DB_MANAGERS: RwLockSync<HashMap<PathBuf, Arc<DbManager>>> =
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

/// A standard way to create a representation of an LMDB environment suitable for Holochain
pub fn create_lmdb_env(path: &Path) -> DatabaseResult<Environment> {
    let mut map = ENVIRONMENTS.write();
    let env: Environment = match map.entry(path.into()) {
        hash_map::Entry::Occupied(e) => e.get().clone(),
        hash_map::Entry::Vacant(e) => e
            .insert({
                let rkv = rkv_builder(None, None)(path)?;
                Environment(Arc::new(RwLock::new(rkv)))
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

#[derive(Shrinkwrap)]
#[shrinkwrap(mutable, unsafe_ignore_visibility)]
pub(crate) struct EnvReadRef<'a, T: 'a> {
    #[shrinkwrap(main_field)]
    data: T,
    guard: RwLockReadGuard<'a, Rkv>,
}

impl<'a, T: 'a> EnvReadRef<'a, T> {
    fn from_parts(data: T, guard: RwLockReadGuard<'a, Rkv>) -> Self {
        Self { data, guard }
    }

    // pub async fn new<F>(mutex: &'a RwLock<Rkv>, f: F) -> Result<EnvReadRef<'a, T>, DatabaseError>
    // where
    //     F: FnOnce(&RwLockReadGuard<'a, Rkv>) -> Result<T, DatabaseError>,
    // {
    //     let guard = mutex.read().await;
    //     let data = f(&guard)?;
    //     Ok(Self { data, guard })
    // }
}

#[derive(Shrinkwrap)]
pub(crate) struct EnvWriteRef<'a, T: 'a> {
    #[shrinkwrap(main_field)]
    data: T,
    guard: RwLockWriteGuard<'a, Rkv>,
}

impl<'a, T: 'a> EnvWriteRef<'a, T> {
    fn from_parts(data: T, guard: RwLockWriteGuard<'a, Rkv>) -> Self {
        Self { data, guard }
    }

    // pub async fn new<F>(mutex: &'a RwLock<Rkv>, f: F) -> Result<EnvWriteRef<'a, T>, DatabaseError>
    // where
    //     F: FnOnce(&RwLockWriteGuard<'a, Rkv>) -> Result<&'a T, DatabaseError>,
    // {
    //     let guard = mutex.write().await;
    //     let data = f(&guard)?;
    //     Ok(Self { data, guard })
    // }
}

/// The canonical representation of a (singleton) LMDB environment.
/// The wrapper contains methods for managing transactions and database connections,
/// tucked away into separate traits.
#[derive(Clone)]
pub struct Environment(Arc<RwLock<Rkv>>);

impl Environment {
    pub async fn guard(&self) -> EnvironmentRef<'_> {
        EnvironmentRef(self.0.read().await)
    }
}

pub struct EnvironmentRef<'e>(RwLockReadGuard<'e, Rkv>);

#[async_trait]
pub trait ReadManager<'e> {
    async fn reader(&'e self) -> DatabaseResult<Reader<'e>>;

    async fn with_reader<E, R, F: Send>(&self, f: F) -> Result<R, E>
    where
        E: From<DatabaseError>,
        F: FnOnce(Reader) -> Result<R, E>;
}

#[async_trait]
pub trait WriteManager<'e> {
    async fn writer(&'e self) -> DatabaseResult<Writer<'e>>;

    async fn with_commit<E, R, F: Send>(&self, f: F) -> Result<R, E>
    where
        E: From<DatabaseError>,
        F: FnOnce(&mut Writer) -> Result<R, E>;
}

#[async_trait]
impl<'e> ReadManager<'e> for EnvironmentRef<'e> {
    async fn reader(&'e self) -> DatabaseResult<Reader<'e>> {
        let reader = Reader::from(ThreadsafeRkvReader::from(self.0.read()?));
        Ok(reader)
    }

    async fn with_reader<E, R, F: Send>(&self, f: F) -> Result<R, E>
    where
        E: From<DatabaseError>,
        F: FnOnce(Reader) -> Result<R, E>,
    {
        f(self.reader().await?)
    }
}

#[async_trait]
impl<'e> WriteManager<'e> for EnvironmentRef<'e> {
    async fn writer(&'e self) -> DatabaseResult<Writer<'e>> {
        let writer = Writer::from(self.0.write()?);
        Ok(writer)
    }

    async fn with_commit<E, R, F: Send>(&self, f: F) -> Result<R, E>
    where
        E: From<DatabaseError>,
        F: FnOnce(&mut Writer) -> Result<R, E>,
    {
        let mut writer = self.writer().await?;
        let result = f(&mut writer);
        writer.commit().map_err(Into::into)?;
        result
    }
}

impl Environment {
    pub async fn inner<'e>(&'e self) -> RwLockReadGuard<'e, Rkv> {
        self.0.read().await
    }

    pub async fn dbs(&self) -> DatabaseResult<Arc<DbManager>> {
        let mut map = DB_MANAGERS.write();
        let dbs = match map.entry(self.inner().await.path().into()) {
            hash_map::Entry::Occupied(e) => e.get().clone(),
            hash_map::Entry::Vacant(e) => {
                e.insert(Arc::new(DbManager::new(self.clone()).await.expect("TODO"))).clone()
            }
        };
        Ok(dbs)
    }
}
