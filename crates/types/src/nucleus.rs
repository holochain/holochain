//! Types related to making calls into Zomes.

use crate::{agent::AgentId, cell::CellId, prelude::*, shims::*};

/// The ZomeId is a pair of CellId and ZomeName.
pub type ZomeId = (CellId, ZomeName);

/// ZomeName as a String (should this be a newtype?)
pub type ZomeName = String;

/// A top-level call into a zome function,
/// i.e. coming from outside the Cell from an external Interface
#[allow(missing_docs)] // members are self-explanitory
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct ZomeInvocation {
    pub cell_id: CellId,
    pub zome_name: ZomeName,
    pub cap: CapabilityRequest,
    pub fn_name: String,
    pub args: JsonString,
    pub provenance: AgentId,
    pub as_at: Address,
}

/// Is this a stub??
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct ZomeInvocationResponse;
