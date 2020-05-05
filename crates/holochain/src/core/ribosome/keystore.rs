use super::HostContext;
use super::WasmRibosome;
use crate::core::ribosome::error::RibosomeResult;
use holochain_zome_types::KeystoreInput;
use holochain_zome_types::KeystoreOutput;
use std::sync::Arc;

pub async fn keystore(
    _ribosome: Arc<WasmRibosome>,
    _host_context: Arc<HostContext>,
    _input: KeystoreInput,
) -> RibosomeResult<KeystoreOutput> {
    unimplemented!();
}
