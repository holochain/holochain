use crate::core::ribosome::error::RibosomeResult;
use crate::core::ribosome::host_fn::HostContext;
use crate::core::ribosome::wasm_ribosome::WasmRibosome;
use holochain_zome_types::KeystoreInput;
use holochain_zome_types::KeystoreOutput;
use std::sync::Arc;

pub async fn keystore(
    _ribosome: Arc<WasmRibosome<'_>>,
    _host_context: Arc<HostContext>,
    _input: KeystoreInput,
) -> RibosomeResult<KeystoreOutput> {
    unimplemented!();
}
