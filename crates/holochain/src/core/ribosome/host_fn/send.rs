use crate::core::ribosome::error::RibosomeResult;
use crate::core::ribosome::host_fn::HostContext;
use crate::core::ribosome::wasm_ribosome::WasmRibosome;
use holochain_zome_types::SendInput;
use holochain_zome_types::SendOutput;
use std::sync::Arc;

pub async fn send(
    _ribosome: Arc<WasmRibosome<'_>>,
    _host_context: Arc<HostContext>,
    _input: SendInput,
) -> RibosomeResult<SendOutput> {
    unimplemented!();
}
