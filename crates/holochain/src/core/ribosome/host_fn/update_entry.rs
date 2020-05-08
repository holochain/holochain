use crate::core::ribosome::error::RibosomeResult;
use crate::core::ribosome::host_fn::HostContext;
use crate::core::ribosome::wasm_ribosome::WasmRibosome;
use holochain_zome_types::UpdateEntryInput;
use holochain_zome_types::UpdateEntryOutput;
use std::sync::Arc;

pub async fn update_entry(
    _ribosome: Arc<WasmRibosome<'_>>,
    _host_context: Arc<HostContext<'_>>,
    _input: UpdateEntryInput,
) -> RibosomeResult<UpdateEntryOutput> {
    unimplemented!();
}
