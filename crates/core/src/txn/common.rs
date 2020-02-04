use holochain_persistence_api::cas::content::{Address, AddressableContent};
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
