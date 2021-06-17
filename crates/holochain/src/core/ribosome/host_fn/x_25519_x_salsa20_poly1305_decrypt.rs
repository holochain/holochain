use crate::core::ribosome::CallContext;
use crate::core::ribosome::RibosomeT;
use holochain_keystore::keystore_actor::KeystoreSenderExt;
use holochain_types::prelude::*;
use holochain_wasmer_host::prelude::WasmError;
use std::sync::Arc;

pub fn x_25519_x_salsa20_poly1305_decrypt(
    _ribosome: Arc<impl RibosomeT>,
    call_context: Arc<CallContext>,
    input: X25519XSalsa20Poly1305Decrypt,
) -> Result<Option<XSalsa20Poly1305Data>, WasmError> {
    tokio_helper::block_forever_on(async move {
        call_context
            .host_access
            .keystore()
            .x_25519_x_salsa20_poly1305_decrypt(input)
            .await
    })
    .map_err(|keystore_error| WasmError::Host(keystore_error.to_string()))
}
