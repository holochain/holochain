use crate::{agent::AgentId, cell::CellId, prelude::*, shims::*};

pub type ZomeId = (CellId, ZomeName);
pub type ZomeName = String;

/// A top-level call into a zome function,
/// i.e. coming from outside the Cell from an external Interface
#[derive(Clone, Debug)]
pub struct ZomeInvocation {
    pub cell_id: CellId,
    pub zome_name: ZomeName,
    pub cap: CapabilityRequest,
    pub fn_name: String,
    pub args: JsonString,
    pub provenance: AgentId,
    pub as_at: Address,
}

pub struct ZomeInvocationResponse;
