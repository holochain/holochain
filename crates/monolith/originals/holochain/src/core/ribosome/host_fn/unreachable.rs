use monolith::holochain::core::ribosome::error::RibosomeResult;
use monolith::holochain::core::ribosome::CallContext;
use monolith::holochain::core::ribosome::RibosomeT;
use monolith::holochain_zome_types::UnreachableInput;
use monolith::holochain_zome_types::UnreachableOutput;
use std::sync::Arc;

pub fn unreachable(
    _ribosome: Arc<impl RibosomeT>,
    _call_context: Arc<CallContext>,
    _input: UnreachableInput,
) -> RibosomeResult<UnreachableOutput> {
    unreachable!();
}
