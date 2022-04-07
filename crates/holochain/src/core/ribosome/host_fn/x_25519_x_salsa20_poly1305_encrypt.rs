use crate::core::ribosome::CallContext;
use crate::core::ribosome::RibosomeT;
use holochain_types::prelude::*;
use holochain_wasmer_host::prelude::*;
use std::sync::Arc;
use crate::core::ribosome::HostFnAccess;

pub fn x_25519_x_salsa20_poly1305_encrypt(
    _ribosome: Arc<impl RibosomeT>,
    call_context: Arc<CallContext>,
    input: X25519XSalsa20Poly1305Encrypt,
) -> Result<XSalsa20Poly1305EncryptedData, WasmError> {
    match HostFnAccess::from(&call_context.host_context()) {
        HostFnAccess{ keystore: Permission::Allow, .. } => {
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
            .map_err(|keystore_error| wasm_error!(WasmErrorInner::Host(keystore_error.to_string())))
        },
        _ => unreachable!(),
    }
}

#[cfg(test)]
#[cfg(feature = "slow_tests")]
pub mod wasm_test {

    use crate::fixt::ZomeCallHostAccessFixturator;
    use ::fixt::prelude::*;
    use hdk::prelude::*;
    // use holochain_state::host_fn_workspace::HostFnWorkspace;
    use holochain_wasm_test_utils::TestWasm;

    #[tokio::test(flavor = "multi_thread")]
    async fn invoke_import_x_25519_x_salsa20_poly1305_encrypt_test() {
        let host_access = fixt!(ZomeCallHostAccess, Predictable);
        let alice: X25519PubKey = crate::call_test_ribosome!(
            host_access,
            TestWasm::XSalsa20Poly1305,
            "create_x25519_keypair",
            ()
        ).unwrap();
        let bob: X25519PubKey = crate::call_test_ribosome!(
            host_access,
            TestWasm::XSalsa20Poly1305,
            "create_x25519_keypair",
            ()
        ).unwrap();
        let carol: X25519PubKey = crate::call_test_ribosome!(
            host_access,
            TestWasm::XSalsa20Poly1305,
            "create_x25519_keypair",
            ()
        ).unwrap();

        let data = XSalsa20Poly1305Data::from(vec![1, 2, 3, 4]);

        let encrypt_input =
            X25519XSalsa20Poly1305Encrypt::new(alice.clone(), bob.clone(), data.clone());

        let encrypt_output: XSalsa20Poly1305EncryptedData = crate::call_test_ribosome!(
            host_access,
            TestWasm::XSalsa20Poly1305,
            "x_25519_x_salsa20_poly1305_encrypt",
            encrypt_input
        ).unwrap();

        let decrypt_input =
            holochain_zome_types::x_salsa20_poly1305::X25519XSalsa20Poly1305Decrypt::new(
                bob.clone(),
                alice.clone(),
                encrypt_output.clone(),
            );

        let decrypt_output: Option<XSalsa20Poly1305Data> = crate::call_test_ribosome!(
            host_access,
            TestWasm::XSalsa20Poly1305,
            "x_25519_x_salsa20_poly1305_decrypt",
            decrypt_input
        ).unwrap();

        assert_eq!(decrypt_output, Some(data.clone()),);

        let bad_decrypt_input =
            holochain_zome_types::x_salsa20_poly1305::X25519XSalsa20Poly1305Decrypt::new(
                carol.clone(),
                alice.clone(),
                encrypt_output,
            );
        let bad_decrypt_output: Result<Option<XSalsa20Poly1305Data>, _> = crate::call_test_ribosome!(
            host_access,
            TestWasm::XSalsa20Poly1305,
            "x_25519_x_salsa20_poly1305_decrypt",
            bad_decrypt_input
        );

        assert!(bad_decrypt_output.is_err());
    }
}
