use super::HostContext;
use super::WasmRibosome;
use crate::core::ribosome::RibosomeError;
use std::sync::Arc;
use sx_zome_types::SendInput;
use sx_zome_types::SendOutput;

pub fn send(
    _ribosome: Arc<WasmRibosome>,
    _host_context: Arc<HostContext>,
    _input: SendInput,
) -> Result<SendOutput, RibosomeError> {
    unimplemented!();
}
