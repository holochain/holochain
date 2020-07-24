use crate::core::ribosome::error::RibosomeResult;
use crate::core::ribosome::CallContext;
use crate::core::ribosome::RibosomeT;
use holochain_zome_types::CallInput;
use holochain_zome_types::CallOutput;
use std::sync::Arc;

pub fn call(
    _ribosome: Arc<impl RibosomeT>,
    _host_context: Arc<CallContext>,
    _input: CallInput,
) -> RibosomeResult<CallOutput> {
    unimplemented!();
}
