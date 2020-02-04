/// Authority for current HEAD of source chain. Logs each header hash with
/// sequential numeric index and  tx_index to group entries by bundle. Also
/// flagged as to whether the DHT transforms have been put into
/// Authoried/Publish queue.

#[allow(unused_imports)]
use crate::txn::common::LmdbSettings;
use holochain_persistence_api::txn::*;
use holochain_persistence_lmdb::txn::*;

use crate::{cell::DnaAddress, txn::common::DatabasePath};
use sx_types::agent::AgentId;

// Sequential index == I in the EAVI

#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq, Hash, PartialOrd)]
pub enum QueuedType {
    Authoring,
    Publishing,
}

#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq, Hash, PartialOrd)]
pub enum Attribute {
    TransactionIndex(u64),
    Queued(QueuedType),
}

#[derive(Clone, Debug, Shrinkwrap)]
pub struct SourceChainPersistence(pub LmdbManager<Attribute>);

impl SourceChainPersistence {
    fn create(dna: DnaAddress, agent: AgentId, settings: LmdbSettings) -> SourceChainPersistence {
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
        SourceChainPersistence(manager)
    }

    pub fn new(dna: DnaAddress, agent: AgentId) -> SourceChainPersistence {
        Self::create(dna, agent, LmdbSettings::Normal)
    }

    #[cfg(test)]
    pub fn test(dna: DnaAddress, agent: AgentId) -> SourceChainPersistence {
        Self::create(dna, agent, LmdbSettings::Test)
    }
}

pub type Cursor = <LmdbManager<Attribute> as CursorProvider<Attribute>>::Cursor;
pub type CursorRw = <LmdbManager<Attribute> as CursorProvider<Attribute>>::CursorRw;
