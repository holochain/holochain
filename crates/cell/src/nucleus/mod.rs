use crate::cell::{CellId, ZomeName};
use sx_types::{agent::AgentId, prelude::*, shims::*};

pub mod error;
pub mod zome_api;

/// A top-level call into a zome function,
/// i.e. coming from outside the Cell from an external Interface
pub struct ZomeInvocation {
    pub cell_id: CellId,
    pub zome_name: ZomeName,
    pub cap: CapabilityRequest,
    pub fn_name: String,
    pub args: JsonString,
    pub provenance: AgentId,
    pub as_at: Address,
}

pub struct ZomeInvocationResult;
