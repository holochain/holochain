use crate::core::ribosome::error::RibosomeResult;
use crate::core::ribosome::CallContext;
use crate::core::ribosome::RibosomeT;
use holochain_zome_types::QueryInput;
use holochain_zome_types::QueryOutput;
use std::sync::Arc;

pub fn query(
    _ribosome: Arc<impl RibosomeT>,
    _call_context: Arc<CallContext>,
    _input: QueryInput,
) -> RibosomeResult<QueryOutput> {
    unimplemented!();
}
