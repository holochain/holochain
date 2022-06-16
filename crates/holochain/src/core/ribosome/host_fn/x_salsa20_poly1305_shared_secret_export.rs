use super::*;
use crate::core::ribosome::CallContext;
use crate::core::ribosome::HostFnAccess;
use crate::core::ribosome::RibosomeError;
use crate::core::ribosome::RibosomeT;
use std::sync::Arc;

pub fn x_salsa20_poly1305_shared_secret_export(
    _ribosome: Arc<impl RibosomeT>,
    call_context: Arc<CallContext>,
    input: XSalsa20Poly1305SharedSecretExport,
) -> Result<XSalsa20Poly1305EncryptedData, RuntimeError> {
    match HostFnAccess::from(&call_context.host_context()) {
        HostFnAccess {
            keystore: Permission::Allow,
            ..
        } => {
            tokio_helper::block_forever_on(async move {
                let tag = input.as_key_ref_ref().to_tag();

                let mut s_pk: [u8; 32] = [0; 32];
                s_pk.copy_from_slice(input.as_sender_ref().as_ref());
                let mut r_pk: [u8; 32] = [0; 32];
                r_pk.copy_from_slice(input.as_recipient_ref().as_ref());

                let (nonce, cipher) = call_context
                    .host_context
                    .keystore()
                    .shared_secret_export(tag, s_pk.into(), r_pk.into())
                    .await?;

                holochain_keystore::LairResult::Ok(XSalsa20Poly1305EncryptedData::new(
                    nonce.into(),
                    cipher.to_vec(),
                ))
            })
            .map_err(|keystore_error| wasm_error!(WasmErrorInner::Host(keystore_error.to_string())).into())
        }
        _ => Err(wasm_error!(WasmErrorInner::Host(
            RibosomeError::HostFnPermissions(
                call_context.zome.zome_name().clone(),
                call_context.function_name().clone(),
                "x_salsa20_poly1305_shared_secret_export".into(),
            )
            .to_string(),
        )).into()),
    }
}
