use crate::core::ribosome::CallContext;
use crate::core::ribosome::RibosomeT;
use holochain_types::prelude::*;
use holochain_wasmer_host::prelude::*;
use std::sync::Arc;
use crate::core::ribosome::HostFnAccess;
use crate::core::ribosome::RibosomeError;

pub fn x_25519_x_salsa20_poly1305_decrypt(
    _ribosome: Arc<impl RibosomeT>,
    call_context: Arc<CallContext>,
    input: X25519XSalsa20Poly1305Decrypt,
) -> Result<Option<XSalsa20Poly1305Data>, RuntimeError> {
    match HostFnAccess::from(&call_context.host_context()) {
        HostFnAccess{ keystore_deterministic: Permission::Allow, .. } => {
            tokio_helper::block_forever_on(async move {
                // zome_types too restrictive,
                // causing us to have to clone everything because there's
                // no access to the actual internal data (*$%&^#(*$&^
                let mut s_pk: [u8; 32] = [0; 32];
                s_pk.copy_from_slice(input.as_sender_ref().as_ref());
                let mut r_pk: [u8; 32] = [0; 32];
                r_pk.copy_from_slice(input.as_recipient_ref().as_ref());

                let edata = input.as_encrypted_data_ref();
                let mut nonce: [u8; 24] = [0; 24];
                nonce.copy_from_slice(edata.as_nonce_ref().as_ref());
                let data = edata.as_encrypted_data_ref().to_vec();

                let res = call_context
                    .host_context
                    .keystore()
                    .crypto_box_xsalsa_open(s_pk.into(), r_pk.into(), nonce, data.into())
                    .await?;

                // why is this an Option #&*(*#@&*&????????
                holochain_keystore::LairResult::Ok(Some(res.to_vec().into()))
            })
            .map_err(|keystore_error| wasm_error!(WasmErrorInner::Host(keystore_error.to_string())))
        },
        _ => Err(wasm_error!(WasmErrorInner::Host(RibosomeError::HostFnPermissions(
            call_context.zome.zome_name().clone(),
            call_context.function_name().clone(),
            "x_25519_x_salsa20_poly1305_decrypt".into()
        ).to_string())))
    }
}
