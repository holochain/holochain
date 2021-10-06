use crate::header::ZomeId;
use crate::zome::ZomeName;
use crate::CapGrant;
use holo_hash::AgentPubKey;
use holo_hash::DnaHash;
use holochain_serialized_bytes::prelude::*;
use crate::FunctionName;

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

impl ZomeInfo {
    pub fn new(
        dna_name: String,
        dna_hash: DnaHash,
        zome_name: ZomeName,
        zome_id: ZomeId,
        properties: SerializedBytes,
    ) -> Self {
        Self {
            dna_name,
            dna_hash,
            zome_name,
            zome_id,
            properties,
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
pub struct DnaInfo;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum CallSource {
    Network,
    ClientAPI,
    LocalCell,
    LocalDNAZome,
    Callback,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum CallInfo {
    // This call was called by a host function, either `call` or `call_remote`.
    // - `CallInfo` of the calling function, same as if `call_info` was called
    //    in the calling context. This is nested/recursive as we may be in call
    //    inception.
    // - `AgentPubKey` of the provenance of the caller. This can be different
    //   to the provenance in `CallInfo` e.g. a `call_remote` could have alice
    //   in bob's `CallInfo` and bob is the current provenance on carol's cell.
    // - `ZomeName` of the caller
    // - `FunctionName` of the caller
    // - `CapGrant` used to authorise _this_ call, i.e. NOT the cap grant in
    //   the calling context, the cap grant use to authorize THIS context
    Call(Box<CallInfo>, AgentPubKey, ZomeName, FunctionName, CapGrant),
    // This call originates from outside the conductor.
    // There is a defined provenance and cap grant but no additional calling
    // context relevant to the DNA such as zome and function as the client
    // operates outside the DNA by definition.
    Client(AgentPubKey, CapGrant),
    // This call is a callback.
    // Authorization is meaningless to a callback. The author of the chain is
    // always implied and the callback implementation itself defines the zome
    // and function.
    Callback,
}

impl CallInfo {
    pub fn new(
        source: CallSource,
        provenance: Option<AgentPubKey>,
        cap_claim: Option<CapClaim>,
    ) -> Self {
        Self {
            source,
            provenance,
            cap_claim,
        }
    }
}
