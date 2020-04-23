use super::HostContext;
use super::WasmRibosome;
use std::sync::Arc;
use sx_zome_types::KeystoreInput;
use sx_zome_types::KeystoreOutput;

pub async fn keystore(
    _ribosome: Arc<WasmRibosome>,
    _host_context: Arc<HostContext>,
    _input: KeystoreInput,
) -> KeystoreOutput {
    unimplemented!();
}
