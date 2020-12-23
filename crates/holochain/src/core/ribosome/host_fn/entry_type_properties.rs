use crate::core::ribosome::error::RibosomeResult;
use crate::core::ribosome::CallContext;
use crate::core::ribosome::RibosomeT;
use holochain_types::prelude::*;
use std::sync::Arc;

pub fn entry_type_properties(
    _ribosome: Arc<impl RibosomeT>,
    _call_context: Arc<CallContext>,
    _input: EntryTypePropertiesInput,
) -> RibosomeResult<EntryTypePropertiesOutput> {
    unimplemented!();
}
