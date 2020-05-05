use super::HostContext;
use super::WasmRibosome;
use crate::core::ribosome::error::RibosomeResult;
use holochain_zome_types::UpdateEntryInput;
use holochain_zome_types::UpdateEntryOutput;
use std::sync::Arc;

pub async fn update_entry(
    _ribosome: Arc<WasmRibosome>,
    _host_context: Arc<HostContext>,
    _input: UpdateEntryInput,
) -> RibosomeResult<UpdateEntryOutput> {
    unimplemented!();
}
