use crate::holochain::core::ribosome::error::RibosomeResult;
use crate::holochain::core::ribosome::CallContext;
use crate::holochain::core::ribosome::RibosomeT;
use crate::holochain_zome_types::EncryptInput;
use crate::holochain_zome_types::EncryptOutput;
use std::sync::Arc;

pub fn encrypt(
    _ribosome: Arc<impl RibosomeT>,
    _call_context: Arc<CallContext>,
    _input: EncryptInput,
) -> RibosomeResult<EncryptOutput> {
    unimplemented!();
}
