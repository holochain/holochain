use crate::holochain::core::ribosome::error::RibosomeResult;
use crate::holochain::core::ribosome::CallContext;
use crate::holochain::core::ribosome::RibosomeT;
use crate::holochain_zome_types::CapabilityInfoInput;
use crate::holochain_zome_types::CapabilityInfoOutput;
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
