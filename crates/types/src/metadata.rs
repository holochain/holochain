//! Types for getting and storing metadata

use crate::HeaderHashed;
use crate::Timestamp;
use holo_hash::HeaderHash;
use holochain_serialized_bytes::prelude::*;
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

/// The status of an [Entry] in the Dht
#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq, Serialize, Deserialize)]
pub enum EntryDhtStatus {
    /// This [Entry] has active headers
    Live,
    /// This [Entry] has no headers that have not been deleted
    Dead,
    /// This [Entry] is awaiting validation
    Pending,
    /// This [Entry] has failed validation and will not be served by the DHT
    Rejected,
    /// This [Entry] has taken too long / too many resources to validate, so we gave up
    Abandoned,
    /// **not implemented** There has been a conflict when validating this [Entry]
    Conflict,
    /// **not implemented** The author has withdrawn their publication of this element.
    Withdrawn,
    /// **not implemented** We have agreed to drop this [Entry] content from the system. Header can stay with no entry
    Purged,
}

impl From<HeaderHashed> for TimedHeaderHash {
    fn from(h: HeaderHashed) -> Self {
        let (header, hash) = h.into_inner();
        TimedHeaderHash {
            timestamp: header.timestamp().into(),
            header_hash: hash,
        }
    }
}
