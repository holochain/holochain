use crate::core::ribosome::CallContext;
use crate::core::ribosome::RibosomeT;
use holochain_keystore::keystore_actor::KeystoreSenderExt;
use holochain_types::prelude::*;
use holochain_wasmer_host::prelude::WasmError;
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
                call_context
                    .host_context
                    .keystore()
                    .x_25519_x_salsa20_poly1305_encrypt(input)
                    .await
            })
            .map_err(|keystore_error| WasmError::Host(keystore_error.to_string()))
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
    use holochain_state::host_fn_workspace::HostFnWorkspace;
    use holochain_wasm_test_utils::TestWasm;

    #[tokio::test(flavor = "multi_thread")]
    async fn invoke_import_x_25519_x_salsa20_poly1305_encrypt_test() {
        let test_env = holochain_state::test_utils::test_cell_env();
        let test_cache = holochain_state::test_utils::test_cache_env();
        let env = test_env.env();
        let author = fake_agent_pubkey_1();
        crate::test_utils::fake_genesis(env.clone())
            .await
            .unwrap();
        let workspace = HostFnWorkspace::new(env.clone(), test_cache.env(), author).await.unwrap();

        let mut host_access = fixt!(ZomeCallHostAccess);
        host_access.workspace = workspace;
        let alice: X25519PubKey = crate::call_test_ribosome!(
            host_access,
            TestWasm::XSalsa20Poly1305,
            "create_x25519_keypair",
            ()
        ).unwrap();
        assert_eq!(
            &alice.as_ref(),
            &[
                65, 17, 71, 31, 48, 10, 48, 208, 3, 220, 71, 246, 83, 246, 74, 221, 3, 123, 54, 48,
                160, 192, 179, 207, 115, 6, 19, 53, 233, 231, 167, 75,
            ]
        );
        let bob: X25519PubKey = crate::call_test_ribosome!(
            host_access,
            TestWasm::XSalsa20Poly1305,
            "create_x25519_keypair",
            ()
        ).unwrap();
        assert_eq!(
            &bob.as_ref(),
            &[
                139, 250, 5, 51, 172, 9, 244, 251, 44, 226, 178, 145, 1, 252, 128, 237, 27, 225,
                11, 171, 153, 205, 115, 228, 72, 211, 110, 41, 115, 48, 251, 98
            ],
        );
        let carol: X25519PubKey = crate::call_test_ribosome!(
            host_access,
            TestWasm::XSalsa20Poly1305,
            "create_x25519_keypair",
            ()
        ).unwrap();
        assert_eq!(
            &carol.as_ref(),
            &[
                211, 158, 23, 148, 162, 67, 112, 72, 185, 58, 136, 103, 76, 164, 39, 200, 83, 124,
                57, 64, 234, 36, 102, 209, 80, 32, 77, 68, 108, 242, 71, 41
            ],
        );

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
        let bad_decrypt_output: Option<XSalsa20Poly1305Data> = crate::call_test_ribosome!(
            host_access,
            TestWasm::XSalsa20Poly1305,
            "x_25519_x_salsa20_poly1305_decrypt",
            bad_decrypt_input
        ).unwrap();

        assert_eq!(bad_decrypt_output, None,);
    }
}
