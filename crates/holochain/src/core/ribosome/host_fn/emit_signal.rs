use crate::core::ribosome::error::RibosomeResult;
use crate::core::ribosome::host_fn::HostContext;
use crate::core::ribosome::wasm_ribosome::WasmRibosome;
use holochain_zome_types::EmitSignalInput;
use holochain_zome_types::EmitSignalOutput;
use std::sync::Arc;

pub async fn emit_signal(
    _ribosome: Arc<WasmRibosome<'_>>,
    _host_context: Arc<HostContext<'_>>,
    _input: EmitSignalInput,
) -> RibosomeResult<EmitSignalOutput> {
    unimplemented!();
}
