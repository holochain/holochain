use super::HostContext;
use super::WasmRibosome;
use std::sync::Arc;
use sx_zome_types::GetEntryInput;
use sx_zome_types::GetEntryOutput;

pub async fn get_entry(
    _ribosome: Arc<WasmRibosome>,
    _host_context: Arc<HostContext>,
    _input: GetEntryInput,
) -> GetEntryOutput {
    unimplemented!();
}
