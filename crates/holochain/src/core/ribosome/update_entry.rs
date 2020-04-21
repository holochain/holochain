use super::HostContext;
use super::WasmRibosome;
use std::sync::Arc;
use sx_zome_types::UpdateEntryInput;
use sx_zome_types::UpdateEntryOutput;

pub async fn update_entry(
    _ribosome: Arc<WasmRibosome>,
    _host_context: Arc<HostContext>,
    _input: UpdateEntryInput,
) -> UpdateEntryOutput {
    unimplemented!();
}
