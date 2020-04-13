use super::HostContext;
use super::WasmRibosome;
use crate::core::ribosome::RibosomeError;
use std::sync::Arc;
use sx_zome_types::PropertyInput;
use sx_zome_types::PropertyOutput;

pub fn property(
    _ribosome: Arc<WasmRibosome>,
    _host_context: Arc<HostContext>,
    _input: PropertyInput,
) -> Result<PropertyOutput, RibosomeError> {
    unimplemented!();
}
