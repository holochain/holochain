/// Authority for current HEAD of source chain. Logs each header hash with 
/// sequential numeric index and  tx_index to group entries by bundle. Also
/// flagged as to whether the DHT transforms have been put into 
/// Authoried/Publish queue.

#[allow(unused_imports)]
use holochain_persistence_api::txn::*;
use holochain_persistence_lmdb::txn::*;

use crate::cell::DnaAddress;
use sx_types::agent::AgentId;
use crate::txn::common::DatabasePath;


// Sequential index == I in the EAVI

#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq, Hash, PartialOrd)]
pub enum QueuedType {
    Authoring,
    Publishing
}

#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq, Hash, PartialOrd)]
pub enum Attribute {
    TransactionIndex(u64),
    Queued(QueuedType)
}
impl holochain_persistence_api::eav::Attribute for Attribute {}


#[derive(Clone, Debug, Shrinkwrap)]
pub struct SourceChainPersistence(pub LmdbManager<Attribute>);

impl SourceChainPersistence {
    pub fn new_manager(dna: DnaAddress, agent: AgentId) -> SourceChainPersistence {
        let db_path : DatabasePath = (dna, agent).into();
        let staging_path : Option<String> = None;
  
        let manager = new_manager(
            db_path,
            staging_path,
            None,
            None,
            None,
            None);
        SourceChainPersistence(manager)
    }
}



