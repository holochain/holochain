use crate::nucleus::ribosome::error::RibosomeResult;
use crate::nucleus::ribosome::CallContext;
use crate::nucleus::ribosome::RibosomeT;
use holochain_zome_types::UnreachableInput;
use holochain_zome_types::UnreachableOutput;
use std::sync::Arc;

pub fn unreachable(
    _ribosome: Arc<impl RibosomeT>,
    _call_context: Arc<CallContext>,
    _input: UnreachableInput,
) -> RibosomeResult<UnreachableOutput> {
    unreachable!();
}
