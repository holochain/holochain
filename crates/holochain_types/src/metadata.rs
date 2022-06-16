//! Types for getting and storing metadata

use holo_hash::ActionHash;
use holochain_serialized_bytes::prelude::*;
pub use holochain_zome_types::metadata::EntryDhtStatus;
use holochain_zome_types::{ActionHashed, Timestamp};
use std::collections::BTreeSet;

/// Timestamp of when the action was created with the actions hash.
#[derive(Debug, Hash, PartialOrd, Ord, PartialEq, Eq, Clone, Serialize, Deserialize)]
pub struct TimedActionHash {
    /// Time when this action was created
    pub timestamp: Timestamp,
    /// Hash of the action
    pub action_hash: ActionHash,
}

/// Metadata returned from a GetMeta request.
/// The Ord derive on TimedActionHash means each set is ordered by time.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, SerializedBytes)]
pub struct MetadataSet {
    /// Actions that created or updated an entry.
    /// These are the actions that show the entry exists.
    pub actions: BTreeSet<TimedActionHash>,
    // TODO: Implement after validation
    /// Placeholder
    pub invalid_actions: BTreeSet<TimedActionHash>,
    /// Deletes on an action
    pub deletes: BTreeSet<TimedActionHash>,
    /// Updates on an action or entry
    pub updates: BTreeSet<TimedActionHash>,
    /// The status of an entry from an authority.
    /// This is simply a faster way of determining if
    /// there are any live actions on an entry.
    pub entry_dht_status: Option<EntryDhtStatus>,
}

impl From<ActionHashed> for TimedActionHash {
    fn from(h: ActionHashed) -> Self {
        let (action, hash) = h.into_inner();
        TimedActionHash {
            timestamp: action.timestamp(),
            action_hash: hash,
        }
    }
}

impl From<ActionHash> for TimedActionHash {
    fn from(h: ActionHash) -> Self {
        TimedActionHash {
            timestamp: Timestamp::now(),
            action_hash: h,
        }
    }
}
