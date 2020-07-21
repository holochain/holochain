use crate::core::ribosome::error::RibosomeResult;
use crate::core::ribosome::wasm_ribosome::WasmRibosome;
use crate::core::ribosome::CallContext;
use holochain_zome_types::RemoveEntryInput;
use holochain_zome_types::RemoveEntryOutput;
use std::sync::Arc;

pub fn remove_entry(
    _ribosome: Arc<WasmRibosome>,
    _call_context: Arc<CallContext>,
    _input: RemoveEntryInput,
) -> RibosomeResult<RemoveEntryOutput> {
    unimplemented!();
}
