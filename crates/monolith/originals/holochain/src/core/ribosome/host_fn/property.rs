use monolith::holochain::core::ribosome::error::RibosomeResult;
use monolith::holochain::core::ribosome::CallContext;
use monolith::holochain::core::ribosome::RibosomeT;
use monolith::holochain_zome_types::PropertyInput;
use monolith::holochain_zome_types::PropertyOutput;
use std::sync::Arc;

pub fn property(
    _ribosome: Arc<impl RibosomeT>,
    _call_context: Arc<CallContext>,
    _input: PropertyInput,
) -> RibosomeResult<PropertyOutput> {
    unimplemented!();
}
