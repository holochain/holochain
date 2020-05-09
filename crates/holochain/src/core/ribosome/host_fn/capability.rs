use crate::core::ribosome::error::RibosomeResult;
use crate::core::ribosome::host_fn::HostContext;
use crate::core::ribosome::wasm_ribosome::WasmRibosome;
use holochain_zome_types::CapabilityInput;
use holochain_zome_types::CapabilityOutput;
use std::sync::Arc;

pub async fn capability(
    _ribosome: Arc<WasmRibosome>,
    _host_context: Arc<HostContext>,
    _input: CapabilityInput,
) -> RibosomeResult<CapabilityOutput> {
    unimplemented!();
}
