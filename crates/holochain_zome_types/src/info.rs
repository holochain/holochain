use crate::header::ZomeId;
use crate::zome::FunctionName;
use crate::zome::ZomeName;
use crate::CapGrant;
use holo_hash::AgentPubKey;
use holo_hash::DnaHash;
use holochain_serialized_bytes::prelude::*;

/// The properties of the current dna/zome being called.
#[allow(missing_docs)]
#[derive(Clone, Debug, Serialize, Deserialize, SerializedBytes, PartialEq)]
pub struct ZomeInfo {
    pub dna_name: String,
    pub dna_hash: DnaHash,
    pub zome_name: ZomeName,
    pub function_name: FunctionName,
    /// The position of this zome in the `dna.json`
    pub zome_id: ZomeId,
}

impl ZomeInfo {
    pub fn new(
        dna_name: String,
        dna_hash: DnaHash,
        zome_name: ZomeName,
        zome_id: ZomeId,
        function_name: FunctionName,
    ) -> Self {
        Self {
            dna_name,
            dna_hash,
            zome_name,
            zome_id,
            function_name,
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
pub struct AppInfo {
    // (nick, hash)
    dnas: Vec<(String, DnaHash)>,
    pub properties: SerializedBytes,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DnaInfo {
    pub dna_hash: DnaHash,
    pub properties: SerializedBytes,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum CallSource {
    /// As long as our call sources never crosses to a different cell, i.e. the
    /// zome may be the same or different within a single DNA/Agency pairing,
    /// we can trust the CallSource recursively.
    LocalCell(Box<CallSource>, ZomeInfo),
    /// As long as our call sources never crosses to a cell of different agency
    /// we can recursively keep a pseudo-backtrace of sorts without leaking
    /// any information between agents and without invalidating the associated
    /// provenance and cap_grant on the `CallInfo`.
    /// - Box<CallSource> => The `CallSource` as it looks to the caller
    /// `ZomeInfo` => The `ZomeInfo` as it looks to the callee
    /// The nick is configured/mapped at happ bundle install time and resolves
    /// at call time.
    LocalDnaNick(Box<CallSource>, ZomeInfo),
    /// This call originated from a different happ BUT with the same agency AND
    /// installed on the same conductor. The `ZomeInfo` as it looks to the
    /// caller isn't relevant in the current happ so it is ommitted.
    Dna(DnaHash, ZomeInfo),
    /// This call originated from the current network from EITHER the current
    /// agent OR a different agent. The provenance and cap grant will give us
    /// this information. Even though the call originated from the current
    /// network we cannot include a recursive `CallSource` stack without
    /// potentially leaking information between agents, and even if we wanted
    /// to it is not possible to verify the information as agents can simply
    /// lie about their call stack, so all we can say for sure is that the call
    /// came from the network.
    Network,
    /// This call originated from a client calling in to the conductor. Client
    /// calls must also provide a provenance and cap grant. There is no call
    /// stack because the client is not calling from within a zomed context.
    Client,
    /// This call is a callback that is initiated by the conductor on behalf of
    /// the current agent. While some zome call may have "triggered" it such as
    /// a post commit callback, every callback runs in its own context with a
    /// fresh workspace and has no direct coupling to its zome call.
    /// The provenance and cap grant is always the author.
    Callback,
    /// This is a zome call initiated by the scheduler. The provenance and cap
    /// grant is always the author even if the schedule was set by some other
    /// agent in a remote call. Incidentally, this is why scheduled functions
    /// don't accept any arguments, to avoid the "confused deputy" security
    /// concern where authentication/permissions are lost across call contexts.
    Scheduled,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CallInfo {
    source: CallSource,
    cap_grant: CapGrant,
}

impl CallInfo {
    pub fn new(source: CallSource, cap_grant: CapGrant, provenance: AgentPubKey) -> Self {
        Self {
            source,
            cap_grant,
            provenance,
        }
    }
}
