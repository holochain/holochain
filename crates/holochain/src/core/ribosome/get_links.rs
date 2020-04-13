use super::HostContext;
use super::WasmRibosome;
use std::sync::Arc;
use sx_zome_types::GetLinksInput;
use sx_zome_types::GetLinksOutput;

pub fn get_links(
    _ribosome: Arc<WasmRibosome>,
    _host_context: Arc<HostContext>,
    _input: GetLinksInput,
) -> GetLinksOutput {
    unimplemented!();
}
