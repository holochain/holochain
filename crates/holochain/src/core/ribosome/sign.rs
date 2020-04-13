use super::HostContext;
use super::WasmRibosome;
use crate::core::ribosome::RibosomeError;
use std::sync::Arc;
use sx_zome_types::SignInput;
use sx_zome_types::SignOutput;

pub fn sign(
    _ribosome: Arc<WasmRibosome>,
    _host_context: Arc<HostContext>,
    _input: SignInput,
) -> Result<SignOutput, RibosomeError> {
    unimplemented!();
}
