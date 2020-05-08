use crate::core::ribosome::error::RibosomeResult;
use crate::core::ribosome::host_fn::HostContext;
use crate::core::ribosome::wasm_ribosome::WasmRibosome;
use holochain_zome_types::EncryptInput;
use holochain_zome_types::EncryptOutput;
use std::sync::Arc;

pub async fn encrypt(
    _ribosome: Arc<WasmRibosome<'_>>,
    _host_context: Arc<HostContext<'_>>,
    _input: EncryptInput,
) -> RibosomeResult<EncryptOutput> {
    unimplemented!();
}
