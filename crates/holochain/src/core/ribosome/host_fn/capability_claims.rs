use crate::core::ribosome::{CallContext, Ribosome};
use std::sync::Arc;
use wasmer::RuntimeError;

/// lists all the local claims filtered by tag
pub fn capability_claims(
    _ribosome: Arc<Ribosome>,
    _call_context: Arc<CallContext>,
    _input: (),
) -> Result<(), RuntimeError> {
    unimplemented!();
}
