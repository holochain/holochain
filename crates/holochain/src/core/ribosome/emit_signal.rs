use super::HostContext;
use super::WasmRibosome;
use std::sync::Arc;
use sx_zome_types::EmitSignalInput;
use sx_zome_types::EmitSignalOutput;

pub fn emit_signal(
    _ribosome: Arc<WasmRibosome>,
    _host_context: Arc<HostContext>,
    _input: EmitSignalInput,
) -> EmitSignalOutput {
    unimplemented!();
}
