use crate::core::ribosome::CallContext;
use crate::core::ribosome::RibosomeT;
use crate::core::{ribosome::error::RibosomeResult, signal::Signal};
use holochain_zome_types::EmitSignalInput;
use holochain_zome_types::EmitSignalOutput;
use std::sync::Arc;

pub fn emit_signal(
    _ribosome: Arc<impl RibosomeT>,
    call_context: Arc<CallContext>,
    input: EmitSignalInput,
) -> RibosomeResult<EmitSignalOutput> {
    tokio_safe_block_on::tokio_safe_block_forever_on(async move {
        let cell_id = todo!("get cell id");
        let bytes = input.into_inner();
        let signal = Signal::App(cell_id, bytes);
        call_context.host_access().signal_tx().send(signal).await
    })?;
    Ok(EmitSignalOutput::new(()))
}
