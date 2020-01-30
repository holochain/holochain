/// Holds Content addressable entries from chains and DHT operational transforms
#[allow(unused_imports)]
use holochain_persistence_api::txn::*;
use holochain_persistence_lmdb::txn::{LmdbManager, new_manager};
use crate::txn::common::DatabasePath;
use crate::cell::DnaAddress;
use sx_types::agent::AgentId;

#[derive(Clone, Debug, Serialize, Deserialize, Hash, Eq, PartialEq, PartialOrd)]
pub enum Attribute { Unimplemented }


impl holochain_persistence_api::eav::Attribute for Attribute {}

#[derive(Clone, Debug, Shrinkwrap)]
pub struct DhtPersistence(pub LmdbManager<Attribute>);

impl DhtPersistence {
    pub fn new_manager(dna: DnaAddress, agent: AgentId) -> DhtPersistence {

        let db_path : DatabasePath = (dna, agent).into();
        let staging_path : Option<String> = None;
        let manager = new_manager(
            db_path,
            staging_path,
            None,
            None,
            None,
            None);
        DhtPersistence(manager)
    }
}



