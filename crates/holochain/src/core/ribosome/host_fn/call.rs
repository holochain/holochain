use crate::core::ribosome::error::RibosomeResult;
use crate::core::ribosome::host_fn::HostContext;
use crate::core::ribosome::wasm_ribosome::WasmRibosome;
use holochain_zome_types::CallInput;
use holochain_zome_types::CallOutput;
use std::sync::Arc;

pub async fn call(
    _ribosome: Arc<WasmRibosome<'_>>,
    _host_context: Arc<HostContext>,
    _input: CallInput,
) -> RibosomeResult<CallOutput> {
    unimplemented!();
}
