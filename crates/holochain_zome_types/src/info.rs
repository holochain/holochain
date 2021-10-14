use crate::header::ZomeId;
use crate::zome::ZomeName;
use holo_hash::AgentPubKey;
use holo_hash::DnaHash;
use holochain_serialized_bytes::prelude::*;

/// The properties of the current dna/zome being called.
#[allow(missing_docs)]
#[derive(Clone, Debug, Serialize, Deserialize, SerializedBytes, PartialEq)]
pub struct ZomeInfo {
    pub name: ZomeName,
    /// The position of this zome in the `dna.json`
    pub id: ZomeId,
}

impl ZomeInfo {
    pub fn new(name: ZomeName, id: ZomeId) -> Self {
        Self { name, id }
    }
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

impl AgentInfo {
    pub fn new(agent_initial_pubkey: AgentPubKey, agent_latest_pubkey: AgentPubKey) -> Self {
        Self {
            agent_initial_pubkey,
            agent_latest_pubkey,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AppInfo;

#[derive(Debug, Serialize, Deserialize)]
pub struct DnaInfo {
    pub name: String,
    pub hash: DnaHash,
    pub properties: SerializedBytes,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CallInfo;
