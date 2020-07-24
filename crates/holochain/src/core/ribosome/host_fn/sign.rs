use crate::core::ribosome::error::RibosomeResult;
use crate::core::ribosome::wasm_ribosome::WasmRibosome;
use crate::core::ribosome::CallContext;
use holochain_zome_types::SignInput;
use holochain_zome_types::SignOutput;
use std::sync::Arc;

pub fn sign(
    _ribosome: Arc<WasmRibosome>,
    _call_context: Arc<CallContext>,
    _input: SignInput,
) -> RibosomeResult<SignOutput> {
    unimplemented!();
}
