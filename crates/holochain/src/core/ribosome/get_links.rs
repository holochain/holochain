use super::HostContext;
use super::WasmRibosome;
use holochain_zome_types::GetLinksInput;
use holochain_zome_types::GetLinksOutput;
use std::sync::Arc;

pub async fn get_links(
    _ribosome: Arc<WasmRibosome>,
    _host_context: Arc<HostContext>,
    _input: GetLinksInput,
) -> GetLinksOutput {
    unimplemented!();
}
