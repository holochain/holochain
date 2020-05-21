use crate::core::ribosome::error::RibosomeResult;
use crate::core::ribosome::wasm_ribosome::WasmRibosome;
use crate::core::ribosome::HostContext;
use holochain_zome_types::PropertyInput;
use holochain_zome_types::PropertyOutput;
use std::sync::Arc;

pub async fn property(
    _ribosome: Arc<WasmRibosome>,
    _host_context: Arc<HostContext>,
    _input: PropertyInput,
) -> RibosomeResult<PropertyOutput> {
    unimplemented!();
}
