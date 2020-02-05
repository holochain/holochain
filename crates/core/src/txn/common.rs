use holochain_persistence_api::cas::content::{Address, AddressableContent};
use lmdb::EnvironmentFlags;
use std::path::{Path, PathBuf};
use sx_types::agent::AgentId;

#[derive(Clone, Debug, Shrinkwrap)]
pub struct DatabasePath(PathBuf);

impl From<(Address, AgentId)> for DatabasePath {
    fn from((addr, id): (Address, AgentId)) -> Self {
        let database_path = PathBuf::new()
            .join(format!("{}", id.address()))
            .join(format!("{}", addr));
        DatabasePath(database_path.into())
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
            LmdbSettings::Test => EnvironmentFlags::WRITE_MAP | EnvironmentFlags::NO_SYNC,
        }
    }
}

impl Default for LmdbSettings {
    fn default() -> Self {
        LmdbSettings::Normal
    }
}
