use crate::{
    cell::{CellId, DnaAddress},
    txn::common::{DatabasePath, LmdbSettings},
};
/// Holds Content addressable entries from chains and DHT operational transforms
#[allow(unused_imports)]
use holochain_persistence_api::txn::*;
use holochain_persistence_lmdb::txn::{new_manager, LmdbManager};
use sx_types::agent::AgentId;
use std::path::Path;

#[derive(Clone, Debug, Serialize, Deserialize, Hash, Eq, PartialEq, PartialOrd, Ord)]
pub enum Attribute {
    Unimplemented,
}

#[derive(Clone, Debug, Shrinkwrap)]
pub struct DhtPersistence(pub LmdbManager<Attribute>);

impl DhtPersistence {

    pub fn new(cell_id: CellId) -> DhtPersistence {
        Self::create(cell_id.into(), LmdbSettings::Normal)
    }

    #[cfg(test)]
    pub fn test(path: &Path) -> DhtPersistence {
        Self::create(path.into(), LmdbSettings::Test)
    }

    fn create(db_path: DatabasePath, settings: LmdbSettings) -> DhtPersistence {
        let staging_path: Option<String> = None;
        let manager = new_manager(
            db_path,
            staging_path,
            None,
            None,
            None,
            Some(settings.into()),
        );
        DhtPersistence(manager)
    }

}

pub type Cursor = <LmdbManager<Attribute> as CursorProvider<Attribute>>::Cursor;
pub type CursorRw = <LmdbManager<Attribute> as CursorProvider<Attribute>>::CursorRw;
