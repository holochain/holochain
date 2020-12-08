use crate::holochain::core::ribosome::error::RibosomeResult;
use crate::holochain::core::ribosome::CallContext;
use crate::holochain::core::ribosome::RibosomeT;
use crate::holochain_zome_types::CapabilityClaimsInput;
use crate::holochain_zome_types::CapabilityClaimsOutput;
use std::sync::Arc;

/// lists all the local claims filtered by tag
pub fn capability_claims(
    _ribosome: Arc<impl RibosomeT>,
    _call_context: Arc<CallContext>,
    _input: CapabilityClaimsInput,
) -> RibosomeResult<CapabilityClaimsOutput> {
    unimplemented!();
}
