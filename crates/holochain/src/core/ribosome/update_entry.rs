use super::HostContext;
use super::WasmRibosome;
use crate::core::ribosome::RibosomeError;
use std::sync::Arc;
use sx_zome_types::UpdateEntryInput;
use sx_zome_types::UpdateEntryOutput;

pub fn update_entry(
    _ribosome: Arc<WasmRibosome>,
    _host_context: Arc<HostContext>,
    _input: UpdateEntryInput,
) -> Result<UpdateEntryOutput, RibosomeError> {
    unimplemented!();
}
