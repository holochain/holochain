//! Data needed to make zome calls.
use holochain_serialized_bytes::prelude::*;
use crate::prelude::*;

/// Zome calls need to be signed regardless of how they are called.
/// This defines exactly what needs to be signed.
#[derive(Serialize, Debug)]
pub struct ZomeCallUnsigned {
    /// Provenance to sign.
    pub provenance: AgentPubKey,
    /// Cell ID to sign.
    pub cell_id: CellId,
    /// Zome name to sign.
    pub zome_name: ZomeName,
    /// Function name to sign.
    pub fn_name: FunctionName,
    /// Cap secret to sign.
    pub cap_secret: Option<CapSecret>,
    /// Payload to sign.
    pub payload: ExternIO,
}