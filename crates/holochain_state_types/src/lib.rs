use holo_hash::ActionHash;
use holochain_integrity_types::prelude::{Action, Entry, Signature};
use serde::{Deserialize, Serialize};

// TODO fix this.  We shouldn't really have nil values but this would
// show if the database is corrupted and doesn't have a record
#[derive(Serialize, Debug, Clone, Deserialize, PartialEq, Eq)]
#[cfg_attr(feature = "ts_rs", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts_rs", ts(export_to = "api/admin/types.ts"))]
pub struct SourceChainDump {
    pub records: Vec<SourceChainDumpRecord>,
    pub published_ops_count: usize,
}

#[derive(Serialize, Debug, Clone, Deserialize, PartialEq, Eq)]
#[cfg_attr(feature = "ts_rs", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts_rs", ts(export_to = "api/admin/types.ts"))]
pub struct SourceChainDumpRecord {
    pub signature: Signature,
    pub action_address: ActionHash,
    pub action: Action,
    pub entry: Option<Entry>,
}

/// Identifies the last source-chain record returned by a paginated dump.
///
/// The next page starts strictly after the identified record.
#[derive(Serialize, Debug, Clone, Deserialize, PartialEq, Eq)]
#[cfg_attr(feature = "ts_rs", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts_rs", ts(export_to = "api/admin/types.ts"))]
pub enum SourceChainCursor {
    /// Resume after this action sequence number.
    Sequence(u32),
    /// Resume after the accepted action identified by this hash.
    ActionHash(ActionHash),
}

pub mod prelude {
    pub use crate::*;
}
