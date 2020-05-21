use crate::hash::HashString;
use crate::zome::ZomeName;
use holochain_serialized_bytes::prelude::*;

#[derive(Clone, Debug, Serialize, Deserialize, SerializedBytes, PartialEq)]
pub struct ZomeGlobals {
    pub dna_name: String,
    pub dna_address: HashString,
    pub zome_name: ZomeName,
    pub agent_id_str: String,
    pub agent_address: HashString,
    pub agent_initial_hash: HashString,
    pub agent_latest_hash: HashString,
    pub public_token: HashString,
    // @todo
    // pub cap_request: Option<CapabilityRequest>,
    pub properties: crate::SerializedBytes,
}
