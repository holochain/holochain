use crate::core::ribosome::error::RibosomeResult;
use crate::core::ribosome::host_fn::HostContext;
use crate::core::ribosome::wasm_ribosome::WasmRibosome;
use holochain_zome_types::EntryTypePropertiesInput;
use holochain_zome_types::EntryTypePropertiesOutput;
use std::sync::Arc;

pub async fn entry_type_properties(
    _ribosome: Arc<WasmRibosome>,
    _host_context: Arc<HostContext>,
    _input: EntryTypePropertiesInput,
) -> RibosomeResult<EntryTypePropertiesOutput> {
    unimplemented!();
}
