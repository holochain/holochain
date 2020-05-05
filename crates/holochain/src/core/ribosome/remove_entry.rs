use super::HostContext;
use super::WasmRibosome;
use crate::core::ribosome::error::RibosomeResult;
use holochain_zome_types::RemoveEntryInput;
use holochain_zome_types::RemoveEntryOutput;
use std::sync::Arc;

pub async fn remove_entry(
    _ribosome: Arc<WasmRibosome>,
    _host_context: Arc<HostContext>,
    _input: RemoveEntryInput,
) -> RibosomeResult<RemoveEntryOutput> {
    unimplemented!();
}
