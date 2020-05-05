use super::HostContext;
use super::WasmRibosome;
use crate::core::ribosome::error::RibosomeResult;
use holochain_zome_types::GetEntryInput;
use holochain_zome_types::GetEntryOutput;
use std::sync::Arc;

pub async fn get_entry(
    _ribosome: Arc<WasmRibosome>,
    _host_context: Arc<HostContext>,
    _input: GetEntryInput,
) -> RibosomeResult<GetEntryOutput> {
    unimplemented!();
}
