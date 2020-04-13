use super::HostContext;
use super::WasmRibosome;
use crate::core::ribosome::RibosomeError;
use std::sync::Arc;
use sx_zome_types::EncryptInput;
use sx_zome_types::EncryptOutput;

pub fn encrypt(
    _ribosome: Arc<WasmRibosome>,
    _host_context: Arc<HostContext>,
    _input: EncryptInput,
) -> Result<EncryptOutput, RibosomeError> {
    unimplemented!();
}
