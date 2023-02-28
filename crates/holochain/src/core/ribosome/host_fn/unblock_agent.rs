use crate::core::ribosome::CallContext;
use crate::core::ribosome::RibosomeT;
use std::sync::Arc;
use holochain_wasmer_host::prelude::*;
use holochain_zome_types::block::Block;
use holochain_zome_types::block::BlockTarget;
use holochain_zome_types::block::CellBlockReason;
use holochain_types::prelude::*;

pub fn unblock_agent(
    _ribosome: Arc<impl RibosomeT>,
    call_context: Arc<CallContext>,
    input: holochain_zome_types::block::BlockAgentInput,
) -> Result<(), RuntimeError> {
    tokio_helper::block_forever_on(async move {
        call_context.host_context().call_zome_handle().unblock(Block::try_new(
            BlockTarget::Cell(call_context
                .host_context()
                .call_zome_handle()
                .cell_id()
                .clone(), CellBlockReason::App(input.reason)),
            input.start,
            input.end
            ).map_err(|e| -> RuntimeError {
                wasm_error!(e.to_string()).into()
            })?).await.map_err(|e| -> RuntimeError {
            wasm_error!(e.to_string()).into()
        })
    })
}
