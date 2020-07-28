use crate::core::ribosome::error::RibosomeResult;
use crate::core::ribosome::CallContext;
use crate::core::ribosome::RibosomeT;
use holochain_zome_types::KeystoreInput;
use holochain_zome_types::KeystoreOutput;
use std::sync::Arc;

pub fn keystore(
    _ribosome: Arc<impl RibosomeT>,
    _call_context: Arc<CallContext>,
    _input: KeystoreInput,
) -> RibosomeResult<KeystoreOutput> {
    unimplemented!();
}
