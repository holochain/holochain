use crate::core::ribosome::CallContext;
use crate::core::ribosome::RibosomeT;
use std::sync::Arc;
use holochain_wasmer_host::prelude::*;

pub fn block_agent(
    _ribosome: Arc<impl RibosomeT>,
    _call_context: Arc<CallContext>,
    input: BlockAgentInput,
) -> Result<(), RuntimeError> {
    unreachable!();
}
