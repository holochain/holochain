use crate::core::ribosome::error::RibosomeResult;
use crate::core::ribosome::wasm_ribosome::WasmRibosome;
use crate::core::ribosome::HostContext;
use holochain_zome_types::GetEntryInput;
use holochain_zome_types::GetEntryOutput;
use std::sync::Arc;

pub async fn get_entry(
    _ribosome: Arc<WasmRibosome>,
    _host_context: Arc<HostContext>,
    _input: GetEntryInput,
) -> RibosomeResult<GetEntryOutput> {
    unimplemented!();
}
