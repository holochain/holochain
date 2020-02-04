use crate::{cell::DnaAddress, txn::common::DatabasePath};
/// Holds Content addressable entries from chains and DHT operational transforms
#[allow(unused_imports)]
use holochain_persistence_api::txn::*;
use holochain_persistence_lmdb::txn::{new_manager, LmdbManager};
use sx_types::agent::AgentId;

#[derive(Clone, Debug, Serialize, Deserialize, Hash, Eq, PartialEq, PartialOrd)]
pub enum Attribute {
    Unimplemented,
}

#[derive(Clone, Debug, Shrinkwrap)]
pub struct DhtPersistence(pub LmdbManager<Attribute>);

impl DhtPersistence {
    pub fn new_manager(dna: DnaAddress, agent: AgentId) -> DhtPersistence {
        let db_path: DatabasePath = (dna, agent).into();
        let staging_path: Option<String> = None;
        let manager = new_manager(db_path, staging_path, None, None, None, None);
        DhtPersistence(manager)
    }
}

pub type Cursor = <LmdbManager<Attribute> as CursorProvider<Attribute>>::Cursor;
pub type CursorRw = <LmdbManager<Attribute> as CursorProvider<Attribute>>::CursorRw;
