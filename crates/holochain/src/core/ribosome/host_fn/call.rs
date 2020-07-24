use crate::core::ribosome::error::RibosomeResult;
use crate::core::ribosome::wasm_ribosome::WasmRibosome;
use crate::core::ribosome::CallContext;
use holochain_zome_types::CallInput;
use holochain_zome_types::CallOutput;
use std::sync::Arc;

pub fn call(
    _ribosome: Arc<WasmRibosome>,
    _call_context: Arc<CallContext>,
    _input: CallInput,
) -> RibosomeResult<CallOutput> {
    unimplemented!();
}
