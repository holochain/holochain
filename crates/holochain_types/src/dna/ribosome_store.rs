use crate::prelude::*;

/// Key for the in-memory EntryDef store
#[derive(
    Debug, Clone, Eq, PartialEq, Hash, Ord, PartialOrd, Serialize, Deserialize, SerializedBytes,
)]
pub struct EntryDefBufferKey {
    /// The zome to which this entry def belongs
    pub zome: IntegrityZomeDef,
    /// The global index, for ordering
    pub entry_def_position: EntryDefIndex,
}

impl EntryDefBufferKey {
    /// Create a new key
    pub fn new(zome: IntegrityZomeDef, entry_def_position: EntryDefIndex) -> Self {
        Self {
            zome,
            entry_def_position,
        }
    }
}

/// Key for the in-memory RateLimit store
#[derive(
    Debug, Clone, Eq, PartialEq, Hash, Ord, PartialOrd, Serialize, Deserialize, SerializedBytes,
)]
pub struct RateLimitBufferKey {
    /// the dna
    pub dna: DnaHash,
    /// either entry or link id
    pub zome_id: GlobalZomeTypeId,
    // /// local, u8, scoped by zome
    // pub local_bucket_id: RateBucketId,
}

impl RateLimitBufferKey {
    /// Create a new key
    pub fn new(dna: DnaHash, zome_id: GlobalZomeTypeId) -> Self {
        Self {
            dna,
            zome_id,
            // local_bucket_id,
        }
    }
}
