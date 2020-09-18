use crate::core::ribosome::error::RibosomeResult;
use crate::core::ribosome::CallContext;
use crate::core::ribosome::RibosomeT;
use holochain_zome_types::SignInput;
use holochain_zome_types::SignOutput;
use std::sync::Arc;
use holochain_keystore::keystore_actor::KeystoreSenderExt;

pub fn sign(
    _ribosome: Arc<impl RibosomeT>,
    call_context: Arc<CallContext>,
    input: SignInput,
) -> RibosomeResult<SignOutput> {
    Ok(SignOutput::new(tokio_safe_block_on::tokio_safe_block_forever_on(async move {
        call_context.host_access.keystore().sign(input.into_inner()).await
    })?))
}
