//! Types for getting and storing metadata

use crate::timestamp;
use crate::Timestamp;
use holo_hash::HeaderHash;
use holochain_serialized_bytes::prelude::*;
pub use holochain_zome_types::metadata::EntryDhtStatus;
use holochain_zome_types::HeaderHashed;
use std::collections::BTreeSet;

/// Timestamp of when the header was created with the headers hash.
#[derive(Debug, Hash, PartialOrd, Ord, PartialEq, Eq, Clone, Serialize, Deserialize)]
pub struct TimedHeaderHash {
    /// Time when this header was created
    pub timestamp: Timestamp,
    /// Hash of the header
    pub header_hash: HeaderHash,
}

/// Metadata returned from a GetMeta request.
/// The Ord derive on TimedHeaderHash means each set is ordered by time.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, SerializedBytes)]
pub struct MetadataSet {
    /// Headers that created or updated an entry.
    /// These are the headers that show the entry exists.
    pub headers: BTreeSet<TimedHeaderHash>,
    // TODO: Implement after validation
    /// Placeholder
    pub invalid_headers: BTreeSet<TimedHeaderHash>,
    /// Deletes on a header
    pub deletes: BTreeSet<TimedHeaderHash>,
    /// Updates on a header or entry
    pub updates: BTreeSet<TimedHeaderHash>,
    /// The status of an entry from an authority.
    /// This is simply a faster way of determining if
    /// there are any live headers on an entry.
    pub entry_dht_status: Option<EntryDhtStatus>,
}

impl From<HeaderHashed> for TimedHeaderHash {
    fn from(h: HeaderHashed) -> Self {
        let (header, hash) = h.into_inner();
        TimedHeaderHash {
            timestamp: header.timestamp(),
            header_hash: hash,
        }
    }
}

impl From<HeaderHash> for TimedHeaderHash {
    fn from(h: HeaderHash) -> Self {
        TimedHeaderHash {
            timestamp: timestamp::now(),
            header_hash: h,
        }
    }
}
