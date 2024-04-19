use crate::core::ribosome::CallContext;
use crate::core::ribosome::HostFnAccess;
use crate::core::ribosome::RibosomeError;
use crate::core::ribosome::RibosomeT;
use holochain_types::prelude::*;
use holochain_wasmer_host::prelude::*;
use std::sync::Arc;
use wasmer::RuntimeError;

pub fn ed_25519_x_salsa20_poly1305_encrypt(
    _ribosome: Arc<impl RibosomeT>,
    call_context: Arc<CallContext>,
    input: Ed25519XSalsa20Poly1305Encrypt,
) -> Result<XSalsa20Poly1305EncryptedData, RuntimeError> {
    match HostFnAccess::from(&call_context.host_context()) {
        HostFnAccess {
            keystore: Permission::Allow,
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

                let (nonce, cipher) = client.crypto_box_xsalsa_by_sign_pub_key(
                    send.into(),
                    recv.into(),
                    None,
                    input.as_data_ref().as_ref().to_vec().into(),
                ).await?;

                holochain_keystore::LairResult::Ok(XSalsa20Poly1305EncryptedData::new(
                    nonce.into(),
                    cipher.to_vec(),
                ))
            })
            .map_err(|keystore_error| -> RuntimeError {
                wasm_error!(WasmErrorInner::Host(keystore_error.to_string())).into()
            })
        }
        _ => Err(wasm_error!(WasmErrorInner::Host(
            RibosomeError::HostFnPermissions(
                call_context.zome.zome_name().clone(),
                call_context.function_name().clone(),
                "ed_25519_x_salsa20_poly1305_encrypt".into(),
            )
            .to_string(),
        ))
        .into()),
    }
}

#[cfg(test)]
#[cfg(feature = "slow_tests")]
pub mod wasm_test {

    use crate::core::ribosome::wasm_test::RibosomeTestFixture;
    use hdk::prelude::*;
    use holochain_wasm_test_utils::TestWasm;

    #[tokio::test(flavor = "multi_thread")]
    async fn invoke_import_ed_25519_x_salsa20_poly1305_encrypt_decrypt_test() {
        holochain_trace::test_run();
        let RibosomeTestFixture {
            conductor, alice, alice_pubkey, bob_pubkey, ..
        } = RibosomeTestFixture::new(TestWasm::XSalsa20Poly1305).await;

        let data = XSalsa20Poly1305Data::from(vec![1, 2, 3, 4]);

        let encrypt_input = Ed25519XSalsa20Poly1305Encrypt::new(
            alice_pubkey.clone(),
            bob_pubkey.clone(),
            data.clone(),
        );

        let encrypt_output: XSalsa20Poly1305EncryptedData = conductor
            .call(&alice, "ed_25519_x_salsa20_poly1305_encrypt", encrypt_input)
            .await;

        let decrypt_input =
            holochain_zome_types::x_salsa20_poly1305::Ed25519XSalsa20Poly1305Decrypt::new(
                bob_pubkey.clone(),
                alice_pubkey.clone(),
                encrypt_output.clone(),
            );

        let decrypt_output: Option<XSalsa20Poly1305Data> = conductor
            .call(&alice, "ed_25519_x_salsa20_poly1305_decrypt", decrypt_input)
            .await;

        assert_eq!(decrypt_output, Some(data.clone()),);
    }
}
