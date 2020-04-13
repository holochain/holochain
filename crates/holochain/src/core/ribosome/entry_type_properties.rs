use super::HostContext;
use super::WasmRibosome;
use crate::core::ribosome::RibosomeError;
use std::sync::Arc;
use sx_zome_types::EntryTypePropertiesInput;
use sx_zome_types::EntryTypePropertiesOutput;

pub fn entry_type_properties(
    _ribosome: Arc<WasmRibosome>,
    _host_context: Arc<HostContext>,
    _input: EntryTypePropertiesInput,
) -> Result<EntryTypePropertiesOutput, RibosomeError> {
    unimplemented!();
}
