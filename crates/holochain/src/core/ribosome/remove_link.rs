use super::HostContext;
use super::WasmRibosome;
use std::sync::Arc;
use sx_zome_types::RemoveLinkInput;
use sx_zome_types::RemoveLinkOutput;

pub async fn remove_link(
    _ribosome: Arc<WasmRibosome>,
    _host_context: Arc<HostContext>,
    _input: RemoveLinkInput,
) -> RemoveLinkOutput {
    unimplemented!();
}
