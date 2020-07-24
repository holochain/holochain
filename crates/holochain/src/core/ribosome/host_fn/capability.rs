use crate::core::ribosome::error::RibosomeResult;
use crate::core::ribosome::CallContext;
use crate::core::ribosome::RibosomeT;
use holochain_zome_types::CapabilityInput;
use holochain_zome_types::CapabilityOutput;
use std::sync::Arc;

pub fn capability(
    _ribosome: Arc<impl RibosomeT>,
    _call_context: Arc<CallContext>,
    _input: CapabilityInput,
) -> RibosomeResult<CapabilityOutput> {
    unimplemented!();
}
