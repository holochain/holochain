use holochain_integrity_types::{Action, Entry, Signature};
use holo_hash::ActionHash;
use serde::{Serialize, Deserialize};

// TODO fix this.  We shouldn't really have nil values but this would
// show if the database is corrupted and doesn't have a record
#[derive(Serialize, Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct SourceChainJsonDump {
    pub records: Vec<SourceChainJsonRecord>,
    pub published_ops_count: usize,
}

#[derive(Serialize, Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct SourceChainJsonRecord {
    pub signature: Signature,
    pub action_address: ActionHash,
    pub action: Action,
    pub entry: Option<Entry>,
}
