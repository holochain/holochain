use crate::core::ribosome::error::RibosomeResult;
use crate::core::ribosome::CallContext;
use crate::core::ribosome::RibosomeT;
use holochain_zome_types::DecryptInput;
use holochain_zome_types::DecryptOutput;
use std::sync::Arc;

pub fn decrypt(
    _ribosome: Arc<impl RibosomeT>,
    _call_context: Arc<CallContext>,
    _input: DecryptInput,
) -> RibosomeResult<DecryptOutput> {
    unimplemented!();
}
