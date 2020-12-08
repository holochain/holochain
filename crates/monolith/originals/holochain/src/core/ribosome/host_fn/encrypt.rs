use monolith::holochain::core::ribosome::error::RibosomeResult;
use monolith::holochain::core::ribosome::CallContext;
use monolith::holochain::core::ribosome::RibosomeT;
use monolith::holochain_zome_types::EncryptInput;
use monolith::holochain_zome_types::EncryptOutput;
use std::sync::Arc;

pub fn encrypt(
    _ribosome: Arc<impl RibosomeT>,
    _call_context: Arc<CallContext>,
    _input: EncryptInput,
) -> RibosomeResult<EncryptOutput> {
    unimplemented!();
}
