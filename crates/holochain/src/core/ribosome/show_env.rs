use super::HostContext;
use super::WasmRibosome;
use crate::core::ribosome::RibosomeError;
use std::sync::Arc;
use sx_zome_types::ShowEnvInput;
use sx_zome_types::ShowEnvOutput;

pub fn show_env(
    _ribosome: Arc<WasmRibosome>,
    _host_context: Arc<HostContext>,
    _input: ShowEnvInput,
) -> Result<ShowEnvOutput, RibosomeError> {
    unimplemented!();
}
