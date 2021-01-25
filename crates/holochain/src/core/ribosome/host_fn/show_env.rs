use crate::core::ribosome::CallContext;
use crate::core::ribosome::RibosomeT;
use std::sync::Arc;
use crate::core::ribosome::RibosomeError;

pub fn show_env(
    _ribosome: Arc<impl RibosomeT>,
    _call_context: Arc<CallContext>,
    _input: (),
) -> Result<(), RibosomeError> {
    unimplemented!();
}
