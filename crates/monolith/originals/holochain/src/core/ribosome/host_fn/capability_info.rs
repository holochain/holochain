use crate::core::ribosome::error::RibosomeResult;
use crate::core::ribosome::CallContext;
use crate::core::ribosome::RibosomeT;
use holochain_zome_types::CapabilityInfoInput;
use holochain_zome_types::CapabilityInfoOutput;
use std::sync::Arc;

/// return the access info used for this call
/// also return who is originated the call (pubkey)
pub fn capability_info(
    _ribosome: Arc<impl RibosomeT>,
    _call_context: Arc<CallContext>,
    _input: CapabilityInfoInput,
) -> RibosomeResult<CapabilityInfoOutput> {
    unimplemented!();
}
