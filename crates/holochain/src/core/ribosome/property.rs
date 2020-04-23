use super::HostContext;
use super::WasmRibosome;
use std::sync::Arc;
use sx_zome_types::PropertyInput;
use sx_zome_types::PropertyOutput;

pub async fn property(
    _ribosome: Arc<WasmRibosome>,
    _host_context: Arc<HostContext>,
    _input: PropertyInput,
) -> PropertyOutput {
    unimplemented!();
}
