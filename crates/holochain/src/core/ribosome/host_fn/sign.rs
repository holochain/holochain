use crate::core::ribosome::error::RibosomeResult;
use crate::core::ribosome::host_fn::HostContext;
use crate::core::ribosome::wasm_ribosome::WasmRibosome;
use holochain_zome_types::SignInput;
use holochain_zome_types::SignOutput;
use std::sync::Arc;

pub async fn sign(
    _ribosome: Arc<WasmRibosome<'_>>,
    _host_context: Arc<HostContext>,
    _input: SignInput,
) -> RibosomeResult<SignOutput> {
    unimplemented!();
}
