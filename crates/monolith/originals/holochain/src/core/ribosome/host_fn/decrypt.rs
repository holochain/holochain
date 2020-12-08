use crate::holochain::core::ribosome::error::RibosomeResult;
use crate::holochain::core::ribosome::CallContext;
use crate::holochain::core::ribosome::RibosomeT;
use crate::holochain_zome_types::DecryptInput;
use crate::holochain_zome_types::DecryptOutput;
use std::sync::Arc;

pub fn decrypt(
    _ribosome: Arc<impl RibosomeT>,
    _call_context: Arc<CallContext>,
    _input: DecryptInput,
) -> RibosomeResult<DecryptOutput> {
    unimplemented!();
}
