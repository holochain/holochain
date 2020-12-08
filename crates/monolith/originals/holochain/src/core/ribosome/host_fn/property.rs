use crate::holochain::core::ribosome::error::RibosomeResult;
use crate::holochain::core::ribosome::CallContext;
use crate::holochain::core::ribosome::RibosomeT;
use crate::holochain_zome_types::PropertyInput;
use crate::holochain_zome_types::PropertyOutput;
use std::sync::Arc;

pub fn property(
    _ribosome: Arc<impl RibosomeT>,
    _call_context: Arc<CallContext>,
    _input: PropertyInput,
) -> RibosomeResult<PropertyOutput> {
    unimplemented!();
}
