use crate::core::ribosome::error::RibosomeResult;
use crate::core::ribosome::host_fn::HostContext;
use crate::core::ribosome::wasm_ribosome::WasmRibosome;
use holochain_zome_types::LinkEntriesInput;
use holochain_zome_types::LinkEntriesOutput;
use std::sync::Arc;

pub async fn link_entries(
    _ribosome: Arc<WasmRibosome<'_>>,
    _host_context: Arc<HostContext>,
    _input: LinkEntriesInput,
) -> RibosomeResult<LinkEntriesOutput> {
    unimplemented!();
}
