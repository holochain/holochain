use super::HostContext;
use super::WasmRibosome;
use crate::core::ribosome::RibosomeError;
use std::sync::Arc;
use sx_zome_types::RemoveEntryInput;
use sx_zome_types::RemoveEntryOutput;

pub fn remove_entry(
    _ribosome: Arc<WasmRibosome>,
    _host_context: Arc<HostContext>,
    _input: RemoveEntryInput,
) -> Result<RemoveEntryOutput, RibosomeError> {
    unimplemented!();
}
