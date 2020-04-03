//! Types related to making calls into Zomes.

use crate::{agent::AgentId, cell::CellId, dna::capabilities::CapabilityRequest, prelude::*};
use sx_wasm_types::WasmExternResponse;

/// The ZomeId is a pair of CellId and ZomeName.
pub type ZomeId = (CellId, ZomeName);

/// ZomeName as a String (should this be a newtype?)
pub type ZomeName = String;

/// wraps payload so that we are compatible with host::guest::call()
#[derive(Clone, Debug, Serialize, Deserialize, SerializedBytes)]
pub struct ZomeInvocationPayload(SerializedBytes);

impl ZomeInvocationPayload {
    /// Create a payload from serialized data
    pub fn new(bytes: SerializedBytes) -> Self {
        Self(bytes)
    }
}

/// A top-level call into a zome function,
/// i.e. coming from outside the Cell from an external Interface
#[allow(missing_docs)] // members are self-explanitory
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct ZomeInvocation {
    pub cell_id: CellId,
    pub zome_name: ZomeName,
    pub cap: CapabilityRequest,
    pub fn_name: String,
    pub payload: ZomeInvocationPayload,
    pub provenance: AgentId,
    pub as_at: Address,
}

/// Response to a zome invocation
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize, PartialEq)]
pub enum ZomeInvocationResponse {
    /// arbitrary functions exposed by zome devs to the outside world
    ZomeApiFn(WasmExternResponse),
}
