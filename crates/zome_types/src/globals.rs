//! Types related to the set of global variables accessible within a zome

use crate::hash::HashString;
use crate::zome::ZomeName;
use holochain_serialized_bytes::prelude::*;

/// The struct containing all global zome values accessible to a zome
#[allow(missing_docs)]
#[derive(Clone, Debug, Serialize, Deserialize, SerializedBytes, PartialEq)]
pub struct ZomeInfo {
    pub dna_name: String,
    pub dna_address: HashString,
    pub zome_name: ZomeName,
    pub public_token: HashString,
    // @todo
    // pub cap_request: Option<CapabilityRequest>,
    pub properties: crate::SerializedBytes,
}

/// The struct containing all global agent values accessible to a zome
#[allow(missing_docs)]
#[derive(Clone, Debug, Serialize, Deserialize, SerializedBytes, PartialEq)]
pub struct AgentInfo {
    pub agent_id_str: String,
    pub agent_address: HashString,
    pub agent_initial_hash: HashString,
    pub agent_latest_hash: HashString,
}

/*
// FYI, after thinking about it, I think it should be more like this,
// but feel free to delete this any time (@maackle)
#[derive(Clone, Debug, Serialize, Deserialize, SerializedBytes, PartialEq)]
pub struct ZomeInfo {
    /// The name of this DNA
    pub dna_name: String,
    /// The hash of this DNA
    pub dna_hash: DnaHash,
    /// The name of this zome
    pub zome_name: ZomeName,
    /// The address of the current Agent
    pub agent_key: AgentPubKey,
    /// The initial address of the current Agent
    pub agent_initial_key: AgentPubKey,
    /// The latest of the current Agent
    // TODO: how is this different from agent_key?
    pub agent_latest_key: AgentPubKey,
    // @todo
    // pub cap_request: Option<CapabilityRequest>,
    /// The properties which were used to install this DNA
    pub properties: crate::SerializedBytes,
}
*/
