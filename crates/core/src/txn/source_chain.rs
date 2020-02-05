/// Authority for current HEAD of source chain. Logs each header hash with
/// sequential numeric index and  tx_index to group entries by bundle. Also
/// flagged as to whether the DHT transforms have been put into
/// Authoried/Publish queue.
use crate::{cell::CellId, txn::common::LmdbSettings};
use crate::{cell::DnaAddress, txn::common::DatabasePath};
use holochain_persistence_api::txn::*;
use holochain_persistence_lmdb::txn::*;
use std::convert::{TryFrom, TryInto};
use sx_types::{agent::AgentId, prelude::*};

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
    pub fn new(cell_id: CellId) -> SourceChainPersistence {
        Self::create(cell_id, LmdbSettings::Normal)
    }

    #[cfg(test)]
    pub fn test(cell_id: CellId) -> SourceChainPersistence {
        Self::create(cell_id, LmdbSettings::Test)
    }

    fn create(cell_id: CellId, settings: LmdbSettings) -> SourceChainPersistence {
        let db_path: DatabasePath = cell_id.into();
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
}

pub type Cursor = <LmdbManager<Attribute> as CursorProvider<Attribute>>::Cursor;
pub type CursorRw = <LmdbManager<Attribute> as CursorProvider<Attribute>>::CursorRw;
