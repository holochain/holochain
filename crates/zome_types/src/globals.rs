use crate::prelude::*;

#[derive(Debug, Serialize, Deserialize, SerializedBytes, PartialEq)]
pub struct ZomeGlobals {
    pub dna_name: String,
    pub dna_address: Address,
    pub agent_id_str: String,
    pub agent_address: Address,
    pub agent_initial_address: Address,
    pub agent_latest_address: Address,
    pub public_token: Address,
    // @todo
    // pub cap_request: Option<CapabilityRequest>,
    pub properties: SerializedBytes,
}
