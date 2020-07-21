use crate::core::ribosome::error::RibosomeResult;
use crate::core::ribosome::wasm_ribosome::WasmRibosome;
use crate::core::ribosome::CallContext;
use holochain_zome_types::UpdateEntryInput;
use holochain_zome_types::UpdateEntryOutput;
use std::sync::Arc;

pub fn update_entry(
    _ribosome: Arc<WasmRibosome>,
    _call_context: Arc<CallContext>,
    _input: UpdateEntryInput,
) -> RibosomeResult<UpdateEntryOutput> {
    unimplemented!();
}
