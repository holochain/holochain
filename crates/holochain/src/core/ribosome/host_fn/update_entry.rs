use crate::core::ribosome::error::RibosomeResult;
use crate::core::ribosome::CallContext;
use crate::core::ribosome::RibosomeT;
use holochain_zome_types::UpdateEntryInput;
use holochain_zome_types::UpdateEntryOutput;
use std::sync::Arc;

pub fn update_entry(
    _ribosome: Arc<impl RibosomeT>,
    _host_context: Arc<CallContext>,
    _input: UpdateEntryInput,
) -> RibosomeResult<UpdateEntryOutput> {
    unimplemented!();
}
