use crate::cell::CellId;
use holochain_persistence_api::{
    cas::content::{Address, AddressableContent},
    txn::*,
};
use holochain_persistence_lmdb::txn::*;
use lmdb::EnvironmentFlags;
use std::{
    convert::TryFrom,
    fmt::Debug,
    hash::Hash,
    path::{Path, PathBuf},
};
use sx_types::{agent::AgentId, prelude::*};

#[derive(Clone, Debug, Shrinkwrap)]
pub struct DatabasePath(PathBuf);

impl From<CellId> for DatabasePath {
    fn from(id: CellId) -> Self {
        let database_path = PathBuf::new().join(format!("{}", id));
        DatabasePath(database_path.into())
    }
}

impl From<&Path> for DatabasePath {
    fn from(path: &Path) -> Self {
        DatabasePath(path.into())
    }
}

impl AsRef<Path> for DatabasePath {
    fn as_ref(&self) -> &Path {
        self.as_path()
    }
}

#[derive(Clone, Debug)]
pub enum LmdbSettings {
    Normal,
    Test,
}

impl From<LmdbSettings> for EnvironmentFlags {
    fn from(settings: LmdbSettings) -> EnvironmentFlags {
        match settings {
            // Note that MAP_ASYNC is absent here, because it degrades data integrity guarantees
            LmdbSettings::Normal => EnvironmentFlags::WRITE_MAP,
            LmdbSettings::Test => EnvironmentFlags::NO_SYNC,
        }
    }
}

impl Default for LmdbSettings {
    fn default() -> Self {
        LmdbSettings::Normal
    }
}
