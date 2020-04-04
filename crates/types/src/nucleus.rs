//! Types related to making calls into Zomes.

use crate::{agent::AgentId, cell::CellId, dna::capabilities::CapabilityRequest, prelude::*};
use sx_wasm_types::WasmExternResponse;

/// The ZomeId is a pair of CellId and ZomeName.
pub type ZomeId = (CellId, ZomeName);

/// ZomeName as a String (should this be a newtype?)
pub type ZomeName = String;

#[derive(Clone, Debug, Serialize, Deserialize)]
/// wraps payload so that we are compatible with host::guest::call()
pub struct ZomeInvocationPayload(SerializedBytes);

impl TryFrom<ZomeInvocationPayload> for SerializedBytes {
    type Error = SerializedBytesError;
    fn try_from(zome_invocation_payload: ZomeInvocationPayload) -> Result<Self, Self::Error> {
        Ok(zome_invocation_payload.0)
    }
}

impl TryFrom<SerializedBytes> for ZomeInvocationPayload {
    type Error = SerializedBytesError;
    fn try_from(serialized_bytes: SerializedBytes) -> Result<Self, Self::Error> {
        Ok(Self(serialized_bytes))
    }
}

/// A top-level call into a zome function,
/// i.e. coming from outside the Cell from an external Interface
#[allow(missing_docs)] // members are self-explanitory
#[derive(Clone, Debug)]
pub struct ZomeInvocation {
    pub cell_id: CellId,
    pub zome_name: ZomeName,
    pub cap: CapabilityRequest,
    pub fn_name: String,
    pub payload: ZomeInvocationPayload,
    pub provenance: AgentId,
    pub as_at: Address,
}

/// Is this a stub??
#[derive(Debug, PartialEq)]
pub enum ZomeInvocationResponse {
    /// arbitrary functions exposed by zome devs to the outside world
    ZomeApiFn(WasmExternResponse),
}
