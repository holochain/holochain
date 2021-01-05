use crate::core::ribosome::error::RibosomeResult;
use crate::core::ribosome::CallContext;
use crate::core::ribosome::RibosomeT;
use holochain_keystore::keystore_actor::KeystoreSenderExt;
use holochain_zome_types::X25519XSalsa20Poly1305DecryptInput;
use holochain_zome_types::X25519XSalsa20Poly1305DecryptOutput;
use std::sync::Arc;

pub fn x_25519_x_salsa20_poly1305_decrypt(
    _ribosome: Arc<impl RibosomeT>,
    call_context: Arc<CallContext>,
    input: X25519XSalsa20Poly1305DecryptInput,
) -> RibosomeResult<X25519XSalsa20Poly1305DecryptOutput> {
    Ok(X25519XSalsa20Poly1305DecryptOutput::new(
        tokio_safe_block_on::tokio_safe_block_forever_on(async move {
            call_context
                .host_access
                .keystore()
                .x_25519_x_salsa20_poly1305_decrypt(input.into_inner())
                .await
        })?,
    ))
}
