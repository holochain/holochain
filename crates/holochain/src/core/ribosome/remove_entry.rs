use super::HostContext;
use super::WasmRibosome;
use std::sync::Arc;
use sx_zome_types::RemoveEntryInput;
use sx_zome_types::RemoveEntryOutput;

pub async fn remove_entry(
    _ribosome: Arc<WasmRibosome>,
    _host_context: Arc<HostContext>,
    _input: RemoveEntryInput,
) -> RemoveEntryOutput {
    unimplemented!();
}
