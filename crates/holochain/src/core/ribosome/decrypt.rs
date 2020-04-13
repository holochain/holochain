use super::HostContext;
use super::WasmRibosome;
use std::sync::Arc;
use sx_zome_types::DecryptInput;
use sx_zome_types::DecryptOutput;

pub fn decrypt(
    _ribosome: Arc<WasmRibosome>,
    _host_context: Arc<HostContext>,
    _input: DecryptInput,
) -> DecryptOutput {
    unimplemented!();
}
