use crate::core::ribosome::CallContext;
use crate::core::ribosome::RibosomeT;
use std::sync::Arc;
use wasmer::RuntimeError;

/// No-op: block_agent has been removed from the HDK.
/// This function remains for backward compatibility with existing apps.
pub fn block_agent(
    _ribosome: Arc<impl RibosomeT>,
    _call_context: Arc<CallContext>,
    _input: holochain_zome_types::block::BlockAgentInput,
) -> Result<(), RuntimeError> {
    Ok(())
}
