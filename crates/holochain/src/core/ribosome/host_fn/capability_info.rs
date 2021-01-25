use crate::core::ribosome::CallContext;
use crate::core::ribosome::RibosomeT;
use std::sync::Arc;
use crate::core::ribosome::RibosomeError;

/// return the access info used for this call
/// also return who is originated the call (pubkey)
pub fn capability_info(
    _ribosome: Arc<impl RibosomeT>,
    _call_context: Arc<CallContext>,
    _input: (),
) -> Result<(), RibosomeError> {
    unimplemented!();
}
