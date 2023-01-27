use crate::core::ribosome::CallContext;
use crate::core::ribosome::RibosomeT;
use std::sync::Arc;
use holochain_wasmer_host::prelude::*;
use holochain_types::block::Block;
use holochain_types::block::BlockTarget;
use holochain_types::block::CellBlockReason;

pub fn block_agent(
    _ribosome: Arc<impl RibosomeT>,
    call_context: Arc<CallContext>,
    input: holochain_zome_types::block::BlockAgentInput,
) -> Result<(), RuntimeError> {
    let _block: Block = Block {
        target: BlockTarget::Cell(call_context
            .host_context()
            .call_zome_handle()
            .cell_id()
            .clone(), CellBlockReason::App(input.reason)),
        start: input.start,
        end: input.end
    };
    unreachable!();
}
