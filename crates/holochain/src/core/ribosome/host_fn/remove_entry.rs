use crate::core::ribosome::error::RibosomeResult;
use crate::core::ribosome::host_fn::HostContext;
use crate::core::ribosome::wasm_ribosome::WasmRibosome;
use holochain_zome_types::RemoveEntryInput;
use holochain_zome_types::RemoveEntryOutput;
use std::sync::Arc;

pub async fn remove_entry(
    _ribosome: Arc<WasmRibosome<'_>>,
    _host_context: Arc<HostContext<'_>>,
    _input: RemoveEntryInput,
) -> RibosomeResult<RemoveEntryOutput> {
    unimplemented!();
}
