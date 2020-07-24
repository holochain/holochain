use crate::core::ribosome::error::RibosomeResult;
use crate::core::ribosome::wasm_ribosome::WasmRibosome;
use crate::core::ribosome::CallContext;
use holochain_zome_types::KeystoreInput;
use holochain_zome_types::KeystoreOutput;
use std::sync::Arc;

pub fn keystore(
    _ribosome: Arc<WasmRibosome>,
    _call_context: Arc<CallContext>,
    _input: KeystoreInput,
) -> RibosomeResult<KeystoreOutput> {
    unimplemented!();
}
