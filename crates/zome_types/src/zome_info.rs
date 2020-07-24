use crate::zome::ZomeName;
use holo_hash::DnaHash;
use holochain_serialized_bytes::prelude::*;

/// The struct containing all global zome values accessible to a zome
#[allow(missing_docs)]
#[derive(Clone, Debug, Serialize, Deserialize, SerializedBytes, PartialEq)]
pub struct ZomeInfo {
    pub dna_name: String,
    pub dna_hash: DnaHash,
    pub zome_name: ZomeName,
    // @todo what is this?
    // pub public_token: HashString,
    // @todo
    // pub cap_request: Option<CapabilityRequest>,
    pub properties: crate::SerializedBytes,
}
