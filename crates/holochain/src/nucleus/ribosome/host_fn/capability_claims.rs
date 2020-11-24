use crate::nucleus::ribosome::error::RibosomeResult;
use crate::nucleus::ribosome::CallContext;
use crate::nucleus::ribosome::RibosomeT;
use holochain_zome_types::CapabilityClaimsInput;
use holochain_zome_types::CapabilityClaimsOutput;
use std::sync::Arc;

/// lists all the local claims filtered by tag
pub fn capability_claims(
    _ribosome: Arc<impl RibosomeT>,
    _call_context: Arc<CallContext>,
    _input: CapabilityClaimsInput,
) -> RibosomeResult<CapabilityClaimsOutput> {
    unimplemented!();
}
