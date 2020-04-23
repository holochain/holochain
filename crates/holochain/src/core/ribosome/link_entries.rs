use super::HostContext;
use super::WasmRibosome;
use std::sync::Arc;
use sx_zome_types::LinkEntriesInput;
use sx_zome_types::LinkEntriesOutput;

pub async fn link_entries(
    _ribosome: Arc<WasmRibosome>,
    _host_context: Arc<HostContext>,
    _input: LinkEntriesInput,
) -> LinkEntriesOutput {
    unimplemented!();
}
