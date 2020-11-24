use crate::nucleus::ribosome::error::RibosomeResult;
use crate::nucleus::ribosome::CallContext;
use crate::nucleus::ribosome::RibosomeT;
use holochain_zome_types::EntryTypePropertiesInput;
use holochain_zome_types::EntryTypePropertiesOutput;
use std::sync::Arc;

pub fn entry_type_properties(
    _ribosome: Arc<impl RibosomeT>,
    _call_context: Arc<CallContext>,
    _input: EntryTypePropertiesInput,
) -> RibosomeResult<EntryTypePropertiesOutput> {
    unimplemented!();
}
