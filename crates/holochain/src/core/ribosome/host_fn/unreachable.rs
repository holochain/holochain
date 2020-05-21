use crate::core::ribosome::error::RibosomeResult;
use crate::core::ribosome::wasm_ribosome::WasmRibosome;
use crate::core::ribosome::HostContext;
use holochain_zome_types::UnreachableInput;
use holochain_zome_types::UnreachableOutput;
use std::sync::Arc;

pub async fn unreachable(
    _ribosome: Arc<WasmRibosome>,
    _host_context: Arc<HostContext>,
    _input: UnreachableInput,
) -> RibosomeResult<UnreachableOutput> {
    unreachable!();
}
