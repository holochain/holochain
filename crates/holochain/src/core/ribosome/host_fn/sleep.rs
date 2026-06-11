use crate::core::ribosome::CallContext;
use crate::core::ribosome::Ribosome;
use std::sync::Arc;
use wasmer::RuntimeError;

pub fn sleep(
    _ribosome: Arc<Ribosome>,
    _call_context: Arc<CallContext>,
    _input: core::time::Duration,
) -> Result<(), RuntimeError> {
    unimplemented!()
}
