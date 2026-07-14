use holo_hash::ActionHash;
use holochain_integrity_types::{Action, Entry, Signature};
use serde::{Deserialize, Serialize};

// TODO fix this.  We shouldn't really have nil values but this would
// show if the database is corrupted and doesn't have a record
#[derive(Serialize, Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct SourceChainDump {
    pub records: Vec<SourceChainDumpRecord>,
    pub published_ops_count: usize,
}

#[derive(Serialize, Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct SourceChainDumpRecord {
    pub signature: Signature,
    pub action_address: ActionHash,
    pub action: Action,
    pub entry: Option<Entry>,
}

pub mod prelude {
    pub use crate::*;
}
