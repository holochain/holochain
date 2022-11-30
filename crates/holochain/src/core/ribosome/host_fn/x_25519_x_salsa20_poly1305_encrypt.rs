use crate::core::ribosome::CallContext;
use crate::core::ribosome::HostFnAccess;
use crate::core::ribosome::RibosomeError;
use crate::core::ribosome::RibosomeT;
use holochain_types::prelude::*;
use holochain_wasmer_host::prelude::*;
use std::sync::Arc;

pub fn x_25519_x_salsa20_poly1305_encrypt(
    _ribosome: Arc<impl RibosomeT>,
    call_context: Arc<CallContext>,
    input: X25519XSalsa20Poly1305Encrypt,
) -> Result<XSalsa20Poly1305EncryptedData, RuntimeError> {
    match HostFnAccess::from(&call_context.host_context()) {
        HostFnAccess {
            keystore: Permission::Allow,
            ..
        } => {
            tokio_helper::block_forever_on(async move {
                // zome_types too restrictive,
                // causing us to have to clone everything because there's
                // no access to the actual internal data (*$%&^#(*$&^
                let mut s_pk: [u8; 32] = [0; 32];
                s_pk.copy_from_slice(input.as_sender_ref().as_ref());
                let mut r_pk: [u8; 32] = [0; 32];
                r_pk.copy_from_slice(input.as_recipient_ref().as_ref());
                let data = input.as_data_ref().as_ref().to_vec();

                let (nonce, cipher) = call_context
                    .host_context
                    .keystore()
                    .crypto_box_xsalsa(s_pk.into(), r_pk.into(), data.into())
                    .await?;

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
                "x_25519_x_salsa20_poly1305_encrypt".into(),
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
    async fn invoke_import_x_25519_x_salsa20_poly1305_encrypt_test() {
        observability::test_run().ok();
        let RibosomeTestFixture {
            conductor, alice, ..
        } = RibosomeTestFixture::new(TestWasm::XSalsa20Poly1305).await;
        let alice_x25519: X25519PubKey = conductor.call(&alice, "create_x25519_keypair", ()).await;
        let bob_x25519: X25519PubKey = conductor.call(&alice, "create_x25519_keypair", ()).await;
        let carol_x25519: X25519PubKey = conductor.call(&alice, "create_x25519_keypair", ()).await;

        let data = XSalsa20Poly1305Data::from(vec![1, 2, 3, 4]);

        let encrypt_input = X25519XSalsa20Poly1305Encrypt::new(
            alice_x25519.clone(),
            bob_x25519.clone(),
            data.clone(),
        );

        let encrypt_output: XSalsa20Poly1305EncryptedData = conductor
            .call(&alice, "x_25519_x_salsa20_poly1305_encrypt", encrypt_input)
            .await;

        let decrypt_input =
            holochain_zome_types::x_salsa20_poly1305::X25519XSalsa20Poly1305Decrypt::new(
                bob_x25519.clone(),
                alice_x25519.clone(),
                encrypt_output.clone(),
            );

        let decrypt_output: Option<XSalsa20Poly1305Data> = conductor
            .call(&alice, "x_25519_x_salsa20_poly1305_decrypt", decrypt_input)
            .await;

        assert_eq!(decrypt_output, Some(data.clone()),);

        let bad_decrypt_input =
            holochain_zome_types::x_salsa20_poly1305::X25519XSalsa20Poly1305Decrypt::new(
                carol_x25519.clone(),
                alice_x25519.clone(),
                encrypt_output,
            );
        let bad_decrypt_output: Result<Option<XSalsa20Poly1305Data>, _> = conductor
            .call_fallible(
                &alice,
                "x_25519_x_salsa20_poly1305_decrypt",
                bad_decrypt_input,
            )
            .await;

        assert!(bad_decrypt_output.is_err());
    }
}
