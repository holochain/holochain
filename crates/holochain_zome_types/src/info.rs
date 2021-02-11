use crate::header::ZomeId;
use crate::zome::ZomeName;
use holo_hash::DnaHash;
use holochain_serialized_bytes::prelude::*;
use holo_hash::AgentPubKey;

/// The properties of the current dna/zome being called.
#[allow(missing_docs)]
#[derive(Clone, Debug, Serialize, Deserialize, SerializedBytes, PartialEq)]
pub struct ZomeInfo {
    pub dna_name: String,
    pub dna_hash: DnaHash,
    pub zome_name: ZomeName,
    /// The position of this zome in the `dna.json`
    pub zome_id: ZomeId,
    pub properties: SerializedBytes,
}

/// The struct containing all information about the executing agent's identity.
#[allow(missing_docs)]
#[derive(Clone, Debug, Serialize, Deserialize, SerializedBytes, PartialEq)]
pub struct AgentInfo {
    /// The current agent's pubkey at genesis.
    /// Always found at index 2 in the source chain.
    pub agent_initial_pubkey: AgentPubKey,
    /// The current agent's current pubkey.
    /// Same as the initial pubkey if it has never been changed.
    /// The agent can revoke an old key and replace it with a new one, the latest appears here.
    pub agent_latest_pubkey: AgentPubKey,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BundleInfo;

#[derive(Debug, Serialize, Deserialize)]
pub struct DnaInfo;

#[derive(Debug, Serialize, Deserialize)]
pub struct CallInfo;