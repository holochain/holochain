use monolith::holochain::core::ribosome::error::RibosomeResult;
use monolith::holochain::core::ribosome::CallContext;
use monolith::holochain::core::ribosome::RibosomeT;
use monolith::holochain_zome_types::CapabilityClaimsInput;
use monolith::holochain_zome_types::CapabilityClaimsOutput;
use std::sync::Arc;

/// lists all the local claims filtered by tag
pub fn capability_claims(
    _ribosome: Arc<impl RibosomeT>,
    _call_context: Arc<CallContext>,
    _input: CapabilityClaimsInput,
) -> RibosomeResult<CapabilityClaimsOutput> {
    unimplemented!();
}
