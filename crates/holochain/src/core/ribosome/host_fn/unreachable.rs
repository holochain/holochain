use crate::core::ribosome::error::RibosomeResult;
use crate::core::ribosome::wasm_ribosome::WasmRibosome;
use crate::core::ribosome::CallContext;
use holochain_zome_types::UnreachableInput;
use holochain_zome_types::UnreachableOutput;
use std::sync::Arc;

pub fn unreachable(
    _ribosome: Arc<WasmRibosome>,
    _call_context: Arc<CallContext>,
    _input: UnreachableInput,
) -> RibosomeResult<UnreachableOutput> {
    unreachable!();
}
