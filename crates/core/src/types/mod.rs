use sx_types::agent::AgentId;
use sx_types::prelude::*;
use sx_types::shims::*;

pub mod net;

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

pub struct UserSignal;
