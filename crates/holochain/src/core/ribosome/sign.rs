use super::HostContext;
use super::WasmRibosome;
use std::sync::Arc;
use sx_zome_types::SignInput;
use sx_zome_types::SignOutput;

pub async fn sign(
    _ribosome: Arc<WasmRibosome>,
    _host_context: Arc<HostContext>,
    _input: SignInput,
) -> SignOutput {
    unimplemented!();
}
