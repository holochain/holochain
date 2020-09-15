use crate::zome::ZomeName;
use holo_hash::DnaHash;
use holochain_serialized_bytes::prelude::*;

/// The properties of the current dna/zome being called.
#[allow(missing_docs)]
#[derive(Clone, Debug, Serialize, Deserialize, SerializedBytes, PartialEq)]
pub struct ZomeInfo {
    pub dna_name: String,
    pub dna_hash: DnaHash,
    pub zome_name: ZomeName,
    pub properties: crate::SerializedBytes,
}
