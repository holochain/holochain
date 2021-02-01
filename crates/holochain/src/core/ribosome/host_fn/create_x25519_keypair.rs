use crate::core::ribosome::error::RibosomeResult;
use crate::core::ribosome::CallContext;
use crate::core::ribosome::RibosomeT;
use holochain_keystore::keystore_actor::KeystoreSenderExt;
use holochain_zome_types::CreateX25519KeypairInput;
use holochain_zome_types::CreateX25519KeypairOutput;
use std::sync::Arc;

pub fn create_x25519_keypair(
    _ribosome: Arc<impl RibosomeT>,
    call_context: Arc<CallContext>,
    _input: CreateX25519KeypairInput,
) -> RibosomeResult<CreateX25519KeypairOutput> {
    Ok(CreateX25519KeypairOutput::new(
        tokio_safe_block_on::tokio_safe_block_forever_on(async move {
            call_context
                .host_access
                .keystore()
                .create_x25519_keypair()
                .await
        })?,
    ))
}

// @see x_25519_x_salsa20_poly1305_encrypt for testing encryption using created keypairs.
