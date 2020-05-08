use crate::core::ribosome::error::RibosomeResult;
use crate::core::ribosome::host_fn::HostContext;
use crate::core::ribosome::wasm_ribosome::WasmRibosome;
use holochain_zome_types::GetEntryInput;
use holochain_zome_types::GetEntryOutput;
use std::sync::Arc;

pub async fn get_entry(
    _ribosome: Arc<WasmRibosome<'_>>,
    _host_context: Arc<HostContext<'_>>,
    _input: GetEntryInput,
) -> RibosomeResult<GetEntryOutput> {
    unimplemented!();
}
