use super::HostContext;
use super::WasmRibosome;
use crate::core::ribosome::error::RibosomeResult;
use holochain_zome_types::ShowEnvInput;
use holochain_zome_types::ShowEnvOutput;
use std::sync::Arc;

pub async fn show_env(
    _ribosome: Arc<WasmRibosome>,
    _host_context: Arc<HostContext>,
    _input: ShowEnvInput,
) -> RibosomeResult<ShowEnvOutput> {
    unimplemented!();
}
