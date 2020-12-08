use monolith::holochain::core::ribosome::error::RibosomeResult;
use monolith::holochain::core::ribosome::CallContext;
use monolith::holochain::core::ribosome::RibosomeT;
use monolith::holochain_zome_types::EntryTypePropertiesInput;
use monolith::holochain_zome_types::EntryTypePropertiesOutput;
use std::sync::Arc;

pub fn entry_type_properties(
    _ribosome: Arc<impl RibosomeT>,
    _call_context: Arc<CallContext>,
    _input: EntryTypePropertiesInput,
) -> RibosomeResult<EntryTypePropertiesOutput> {
    unimplemented!();
}
