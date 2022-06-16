use super::*;
use crate::core::ribosome::CallContext;
use crate::core::ribosome::HostFnAccess;
use crate::core::ribosome::RibosomeError;
use crate::core::ribosome::RibosomeT;
use std::sync::Arc;

pub fn x_salsa20_poly1305_shared_secret_ingest(
    _ribosome: Arc<impl RibosomeT>,
    call_context: Arc<CallContext>,
    input: XSalsa20Poly1305SharedSecretIngest,
) -> Result<XSalsa20Poly1305KeyRef, RuntimeError> {
    match HostFnAccess::from(&call_context.host_context()) {
        HostFnAccess {
            keystore: Permission::Allow,
            ..
        } => {
            tokio_helper::block_forever_on(async move {
                let key_ref = match input.as_key_ref_ref() {
                    Some(key_ref) => key_ref.clone(),
                    None => rand_utf8::rand_utf8(
                        &mut rand::thread_rng(),
                        DEF_REF_SIZE,
                    ).as_bytes().to_vec().into(),
                };

                let tag = key_ref.to_tag();

                let mut s_pk: [u8; 32] = [0; 32];
                s_pk.copy_from_slice(input.as_sender_ref().as_ref());
                let mut r_pk: [u8; 32] = [0; 32];
                r_pk.copy_from_slice(input.as_recipient_ref().as_ref());

                let edata = input.as_encrypted_data_ref();
                let mut nonce: [u8; 24] = [0; 24];
                nonce.copy_from_slice(edata.as_nonce_ref().as_ref());
                let data = edata.as_encrypted_data_ref().to_vec();

                call_context
                    .host_context
                    .keystore()
                    .shared_secret_import(
                        s_pk.into(),
                        r_pk.into(),
                        nonce,
                        data.into(),
                        tag,
                    )
                    .await?;

                holochain_keystore::LairResult::Ok(key_ref)
            })
            .map_err(|keystore_error| wasm_error!(WasmErrorInner::Host(keystore_error.to_string())).into())
        }
        _ => Err(wasm_error!(WasmErrorInner::Host(
            RibosomeError::HostFnPermissions(
                call_context.zome.zome_name().clone(),
                call_context.function_name().clone(),
                "x_salsa20_poly1305_shared_secret_ingest".into(),
            )
            .to_string(),
        )).into()),
    }
}
