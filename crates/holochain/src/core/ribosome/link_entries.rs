use super::HostContext;
use super::WasmRibosome;
use crate::core::ribosome::error::RibosomeResult;
use holochain_zome_types::LinkEntriesInput;
use holochain_zome_types::LinkEntriesOutput;
use std::sync::Arc;

pub async fn link_entries(
    _ribosome: Arc<WasmRibosome>,
    _host_context: Arc<HostContext>,
    _input: LinkEntriesInput,
) -> RibosomeResult<LinkEntriesOutput> {
    unimplemented!();
}
