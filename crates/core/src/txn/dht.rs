use crate::txn::common::LmdbSettings;
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
    fn create(dna: DnaAddress, agent: AgentId, settings: LmdbSettings) -> DhtPersistence {
        let db_path: DatabasePath = (dna, agent).into();
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

    pub fn new(dna: DnaAddress, agent: AgentId) -> DhtPersistence {
        Self::create(dna, agent, LmdbSettings::Normal)
    }

    #[cfg(test)]
    pub fn test(dna: DnaAddress, agent: AgentId) -> DhtPersistence {
        Self::create(dna, agent, LmdbSettings::Test)
    }
}

pub type Cursor = <LmdbManager<Attribute> as CursorProvider<Attribute>>::Cursor;
pub type CursorRw = <LmdbManager<Attribute> as CursorProvider<Attribute>>::CursorRw;
