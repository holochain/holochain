use crate::header::ZomeId;
use crate::zome::ZomeName;
use crate::CapGrant;
use crate::EntryDefs;
use crate::FunctionName;
use crate::Timestamp;
use holo_hash::AgentPubKey;
use holo_hash::DnaHash;
use holo_hash::HeaderHash;
use holochain_serialized_bytes::prelude::*;

/// The properties of the current dna/zome being called.
#[allow(missing_docs)]
#[derive(Clone, Debug, Serialize, Deserialize, SerializedBytes, PartialEq)]
pub struct ZomeInfo {
    pub name: ZomeName,
    /// The position of this zome in the `dna.json`
    pub id: ZomeId,
    pub entry_defs: EntryDefs,
}

impl ZomeInfo {
    pub fn new(name: ZomeName, id: ZomeId, entry_defs: EntryDefs) -> Self {
        Self {
            name,
            id,
            entry_defs,
        }
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
    pub chain_head: (HeaderHash, u32, Timestamp),
}

impl AgentInfo {
    pub fn new(
        agent_initial_pubkey: AgentPubKey,
        agent_latest_pubkey: AgentPubKey,
        chain_head: (HeaderHash, u32, Timestamp),
    ) -> Self {
        Self {
            agent_initial_pubkey,
            agent_latest_pubkey,
            chain_head,
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
    // In ZomeId order as to match corresponding `ZomeInfo` for each.
    pub zome_names: Vec<ZomeName>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CallInfo {
    pub provenance: AgentPubKey,
    pub function_name: FunctionName,
    /// Chain head as at the call start.
    /// This will not change within a call even if the chain is written to.
    pub as_at: (HeaderHash, u32, Timestamp),
    pub cap_grant: CapGrant,
}
