use crate::core::ribosome::error::RibosomeResult;
use crate::core::ribosome::wasm_ribosome::WasmRibosome;
use crate::core::ribosome::CallContext;
use holochain_zome_types::EncryptInput;
use holochain_zome_types::EncryptOutput;
use std::sync::Arc;

pub fn encrypt(
    _ribosome: Arc<WasmRibosome>,
    _call_context: Arc<CallContext>,
    _input: EncryptInput,
) -> RibosomeResult<EncryptOutput> {
    unimplemented!();
}
