use crate::core::ribosome::{CallContext, Ribosome};
use crate::core::ribosome::RibosomeT;
use std::sync::Arc;
use wasmer::RuntimeError;

/// return the access info used for this call
/// also return who is originated the call (pubkey)
pub fn capability_info(
    _ribosome: Arc<Ribosome>,
    _call_context: Arc<CallContext>,
    _input: (),
) -> Result<(), RuntimeError> {
    unimplemented!();
}
