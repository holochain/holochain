use crate::core::ribosome::CallContext;
use crate::core::ribosome::RibosomeT;
use std::sync::Arc;
use crate::core::ribosome::RibosomeError;

pub fn entry_type_properties(
    _ribosome: Arc<impl RibosomeT>,
    _call_context: Arc<CallContext>,
    _input: (),
) -> Result<(), RibosomeError> {
    unimplemented!();
}
