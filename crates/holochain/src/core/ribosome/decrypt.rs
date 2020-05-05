use super::HostContext;
use super::WasmRibosome;
use crate::core::ribosome::error::RibosomeResult;
use holochain_zome_types::DecryptInput;
use holochain_zome_types::DecryptOutput;
use std::sync::Arc;

pub async fn decrypt(
    _ribosome: Arc<WasmRibosome>,
    _host_context: Arc<HostContext>,
    _input: DecryptInput,
) -> RibosomeResult<DecryptOutput> {
    unimplemented!();
}
