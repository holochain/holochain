use crate::core::ribosome::error::RibosomeResult;
use crate::core::ribosome::host_fn::HostContext;
use crate::core::ribosome::wasm_ribosome::WasmRibosome;
use holochain_zome_types::PropertyInput;
use holochain_zome_types::PropertyOutput;
use std::sync::Arc;

pub async fn property(
    _ribosome: Arc<WasmRibosome<'_>>,
    _host_context: Arc<HostContext>,
    _input: PropertyInput,
) -> RibosomeResult<PropertyOutput> {
    unimplemented!();
}
