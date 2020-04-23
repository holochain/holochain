use super::HostContext;
use super::WasmRibosome;
use std::sync::Arc;
use sx_zome_types::ShowEnvInput;
use sx_zome_types::ShowEnvOutput;

pub async fn show_env(
    _ribosome: Arc<WasmRibosome>,
    _host_context: Arc<HostContext>,
    _input: ShowEnvInput,
) -> ShowEnvOutput {
    unimplemented!();
}
