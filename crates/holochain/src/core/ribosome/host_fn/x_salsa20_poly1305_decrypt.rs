use super::*;
use crate::core::ribosome::CallContext;
use crate::core::ribosome::RibosomeT;
use std::sync::Arc;
use crate::core::ribosome::HostFnAccess;
use crate::core::ribosome::RibosomeError;

pub fn x_salsa20_poly1305_decrypt(
    _ribosome: Arc<impl RibosomeT>,
    call_context: Arc<CallContext>,
    input: XSalsa20Poly1305Decrypt,
) -> Result<Option<XSalsa20Poly1305Data>, RuntimeError> {
    match HostFnAccess::from(&call_context.host_context()) {
        HostFnAccess{
            keystore_deterministic: Permission::Allow,
            ..
        } => {
            tokio_helper::block_forever_on(async move {
                let key_ref = input.as_key_ref_ref().clone();
                let tag = key_ref.to_tag();

                let edata = input.as_encrypted_data_ref();
                let mut nonce: [u8; 24] = [0; 24];
                nonce.copy_from_slice(edata.as_nonce_ref().as_ref());
                let data = edata.as_encrypted_data_ref().to_vec();

                // for some reason, the hdk api expects us to translate
                // errors into None here
                let res = match call_context
                    .host_context
                    .keystore()
                    .shared_secret_decrypt(tag, nonce, data.into())
                    .await
                {
                    Err(_) => None,
                    Ok(res) => Some(res.to_vec().into()),
                };
                holochain_keystore::LairResult::Ok(res)
            })
            .map_err(|keystore_error| -> RuntimeError { wasm_error!(WasmErrorInner::Host(keystore_error.to_string())).into() })
        }
        _ => Err(wasm_error!(WasmErrorInner::Host(RibosomeError::HostFnPermissions(
            call_context.zome.zome_name().clone(),
            call_context.function_name().clone(),
            "x_salsa20_poly1305_decrypt".into()
        ).to_string())).into())
    }
}

// Tests for the shared secret round trip are in xsalsa20_poly1305_encrypt.
