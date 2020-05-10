use crate::core::ribosome::error::RibosomeResult;
use crate::core::ribosome::HostContext;
use crate::core::ribosome::wasm_ribosome::WasmRibosome;
use holochain_zome_types::RemoveLinkInput;
use holochain_zome_types::RemoveLinkOutput;
use std::sync::Arc;

pub async fn remove_link(
    _ribosome: Arc<WasmRibosome>,
    _host_context: Arc<HostContext>,
    _input: RemoveLinkInput,
) -> RibosomeResult<RemoveLinkOutput> {
    unimplemented!();
}
