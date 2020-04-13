use super::HostContext;
use super::WasmRibosome;
use crate::core::ribosome::RibosomeError;
use std::sync::Arc;
use sx_zome_types::CapabilityInput;
use sx_zome_types::CapabilityOutput;

pub fn capability(
    _ribosome: Arc<WasmRibosome>,
    _host_context: Arc<HostContext>,
    _input: CapabilityInput,
) -> Result<CapabilityOutput, RibosomeError> {
    unimplemented!();
}
