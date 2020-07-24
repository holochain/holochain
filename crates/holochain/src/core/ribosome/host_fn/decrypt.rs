use crate::core::ribosome::error::RibosomeResult;
use crate::core::ribosome::wasm_ribosome::WasmRibosome;
use crate::core::ribosome::CallContext;
use holochain_zome_types::DecryptInput;
use holochain_zome_types::DecryptOutput;
use std::sync::Arc;

pub fn decrypt(
    _ribosome: Arc<WasmRibosome>,
    _call_context: Arc<CallContext>,
    _input: DecryptInput,
) -> RibosomeResult<DecryptOutput> {
    unimplemented!();
}
