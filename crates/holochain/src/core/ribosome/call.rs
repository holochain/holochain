use super::HostContext;
use super::WasmRibosome;
use std::sync::Arc;
use sx_zome_types::CallInput;
use sx_zome_types::CallOutput;

pub fn call(
    _ribosome: Arc<WasmRibosome>,
    _host_context: Arc<HostContext>,
    _input: CallInput,
) -> CallOutput {
    unimplemented!();
}
