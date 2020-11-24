use crate::core::signal::Signal;
use crate::nucleus::ribosome::error::RibosomeResult;
use crate::nucleus::ribosome::CallContext;
use crate::nucleus::ribosome::RibosomeT;
use holochain_zome_types::EmitSignalInput;
use holochain_zome_types::EmitSignalOutput;
use std::sync::Arc;

pub fn emit_signal(
    _ribosome: Arc<impl RibosomeT>,
    call_context: Arc<CallContext>,
    input: EmitSignalInput,
) -> RibosomeResult<EmitSignalOutput> {
    let cell_id = call_context.host_access().cell_id().clone();
    let bytes = input.into_inner();
    let signal = Signal::App(cell_id, bytes);
    call_context.host_access().signal_tx().send(signal)?;
    Ok(EmitSignalOutput::new(()))
}
