use crate::core::ribosome::CallContext;
use crate::core::ribosome::HostFnAccess;
use crate::core::ribosome::RibosomeError;
use crate::core::ribosome::RibosomeT;
use holochain_types::prelude::*;
use holochain_wasmer_host::prelude::*;
use holochain_wasmer_host::wasm_host_error as wasm_error;
use std::sync::Arc;
use wasmer::RuntimeError;

pub fn ed_25519_x_salsa20_poly1305_decrypt(
    _ribosome: Arc<impl RibosomeT>,
    call_context: Arc<CallContext>,
    input: Ed25519XSalsa20Poly1305Decrypt,
) -> Result<XSalsa20Poly1305Data, RuntimeError> {
    match HostFnAccess::from(&call_context.host_context()) {
        HostFnAccess {
            keystore_deterministic: Permission::Allow,
            ..
        } => {
            tokio_helper::block_forever_on(async move {
                let client = call_context
                    .host_context
                    .keystore()
                    .lair_client();

                let mut send = [0; 32];
                send.copy_from_slice(input.as_sender_ref().get_raw_32());
                let mut recv = [0; 32];
                recv.copy_from_slice(input.as_recipient_ref().get_raw_32());
                let mut nonce = [0; 24];
                nonce.copy_from_slice(input.as_encrypted_data_ref().as_nonce_ref().as_ref());

                let res = client.crypto_box_xsalsa_open_by_sign_pub_key(
                    send.into(),
                    recv.into(),
                    None,
                    nonce,
                    input.as_encrypted_data_ref().as_encrypted_data_ref().to_vec().into(),
                ).await?;

                holochain_keystore::LairResult::Ok(res.to_vec().into())
            })
            .map_err(|keystore_error| -> RuntimeError { wasm_error!(WasmErrorInner::Host(keystore_error.to_string())).into() })
        }
        _ => Err(wasm_error!(WasmErrorInner::Host(
            RibosomeError::HostFnPermissions(
                call_context.zome.zome_name().clone(),
                call_context.function_name().clone(),
                "ed_25519_x_salsa20_poly1305_decrypt".into()
            )
            .to_string()
        ))
        .into()),
    }
}
