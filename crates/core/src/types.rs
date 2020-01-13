use holochain_persistence_api::cas::content::Address;
use holochain_core_types::agent::AgentId;
use holochain_json_api::json::JsonString;
use holochain_core_types::{dna::capabilities::CapabilityRequest};

pub struct ZomeInvocation {
    pub zome_name: String,
    pub cap: CapabilityRequest,
    pub fn_name: String,
    pub parameters: JsonString,
    pub provenance: AgentId,
    pub as_at: Address,
}

pub struct ZomeInvocationResult;

pub enum Signal {
    Trace,
    // Consistency(ConsistencySignal<String>),
    User(UserSignal),
}

type UserSignal = ();
