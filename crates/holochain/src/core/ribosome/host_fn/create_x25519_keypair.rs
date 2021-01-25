use crate::core::ribosome::CallContext;
use crate::core::ribosome::RibosomeT;
use holochain_keystore::keystore_actor::KeystoreSenderExt;
use std::sync::Arc;
use holochain_zome_types::X25519PubKey;
use crate::core::ribosome::RibosomeError;

pub fn create_x25519_keypair(
    _ribosome: Arc<impl RibosomeT>,
    call_context: Arc<CallContext>,
    _input: (),
) -> Result<X25519PubKey, RibosomeError> {
    Ok(
        tokio_safe_block_on::tokio_safe_block_forever_on(async move {
            call_context
                .host_access
                .keystore()
                .create_x25519_keypair()
                .await
        })?,
    )
}

// @see x_25519_x_salsa20_poly1305_encrypt for testing encryption using created keypairs.
