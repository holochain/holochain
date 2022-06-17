use crate::core::ribosome::CallContext;
use crate::core::ribosome::HostFnAccess;
use crate::core::ribosome::RibosomeError;
use crate::core::ribosome::RibosomeT;
use holochain_types::prelude::*;
use holochain_wasmer_host::prelude::*;
use ring::rand::SecureRandom;
use std::convert::TryInto;
use std::sync::Arc;
use xsalsa20poly1305::aead::{generic_array::GenericArray, Aead, NewAead};
use xsalsa20poly1305::XSalsa20Poly1305;

pub fn x_salsa20_poly1305_encrypt(
    _ribosome: Arc<impl RibosomeT>,
    call_context: Arc<CallContext>,
    input: XSalsa20Poly1305Encrypt,
) -> Result<XSalsa20Poly1305EncryptedData, RuntimeError> {
    match HostFnAccess::from(&call_context.host_context()) {
        HostFnAccess {
            keystore: Permission::Allow,
            ..
        } => {
            let system_random = ring::rand::SystemRandom::new();
            let mut nonce_bytes = [0; holochain_zome_types::x_salsa20_poly1305::nonce::NONCE_BYTES];
            system_random
                .fill(&mut nonce_bytes)
                .map_err(|ring_unspecified| -> RuntimeError {
                    wasm_error!(WasmErrorInner::Host(ring_unspecified.to_string())).into()
                })?;

            // @todo use the real libsodium somehow instead of this rust crate.
            // The main issue here is dependency management - it's not necessarily simple to get libsodium
            // reliably on consumer devices, e.g. we might want to statically link it somewhere.
            // @todo this key ref should be an opaque ref to lair and the encrypt should happen in lair.
            let lib_key = GenericArray::from_slice(input.as_key_ref_ref().as_ref());
            let cipher = XSalsa20Poly1305::new(lib_key);
            let lib_nonce = GenericArray::from_slice(&nonce_bytes);
            let lib_encrypted_data = cipher
                .encrypt(lib_nonce, input.as_data_ref().as_ref())
                .map_err(|aead_error| -> RuntimeError {
                    wasm_error!(WasmErrorInner::Host(aead_error.to_string())).into()
                })?;

            Ok(
                holochain_zome_types::x_salsa20_poly1305::encrypted_data::XSalsa20Poly1305EncryptedData::new(
                    match lib_nonce.as_slice().try_into() {
                        Ok(nonce) => nonce,
                        Err(secure_primitive_error) => return Err(wasm_error!(WasmErrorInner::Host(secure_primitive_error.to_string())).into()),
                    },
                    lib_encrypted_data,
                ),
            )
        }
        _ => Err(wasm_error!(WasmErrorInner::Host(
            RibosomeError::HostFnPermissions(
                call_context.zome.zome_name().clone(),
                call_context.function_name().clone(),
                "x_salsa20_poly1305_encrypt".into(),
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
    #[cfg(feature = "test_utils")]
    async fn xsalsa20_poly1305_shared_secret_round_trip() {
        observability::test_run().ok();

        // we need two conductors and two x25519 pub keys to do a round trip

        // conductor1 / pubkey 1
        let RibosomeTestFixture {
            conductor: conductor1, alice: alice1, ..
        } = RibosomeTestFixture::new(TestWasm::XSalsa20Poly1305).await;
        let alice1_x25519: X25519PubKey = conductor1.call(&alice1, "create_x25519_keypair", ()).await;

        // conductor2 / pubkey 2
        let RibosomeTestFixture {
            conductor: conductor2, alice: alice2, ..
        } = RibosomeTestFixture::new(TestWasm::XSalsa20Poly1305).await;
        let alice2_x25519: X25519PubKey = conductor2.call(&alice2, "create_x25519_keypair", ()).await;

        // create a new random shared key
        let key_ref: XSalsa20Poly1305KeyRef = conductor1
            .call(
                &alice1,
                "x_salsa20_poly1305_shared_secret_create_random",
                <Option<XSalsa20Poly1305KeyRef>>::None,
            )
            .await;

        // encrypt some data with that shared key (identified by key_ref)
        let data = XSalsa20Poly1305Data::from(vec![1, 2, 3, 4]);
        let enc_input = holochain_zome_types::x_salsa20_poly1305::XSalsa20Poly1305Encrypt::new(
            key_ref.clone(),
            data.clone(),
        );
        let cipher: XSalsa20Poly1305EncryptedData = conductor1
            .call(&alice1, "x_salsa20_poly1305_encrypt", enc_input)
            .await;

        // export the shared key to send to conductor2
        let exp_input = holochain_zome_types::x_salsa20_poly1305::XSalsa20Poly1305SharedSecretExport::new(
            alice1_x25519.clone(), // sender
            alice2_x25519.clone(), // recipient
            key_ref.clone(),
        );
        let secret_exp: XSalsa20Poly1305EncryptedData = conductor1
            .call(&alice1, "x_salsa20_poly1305_shared_secret_export", exp_input)
            .await;

        // ingest the shared key on conductor2
        let ing_input = holochain_zome_types::x_salsa20_poly1305::XSalsa20Poly1305SharedSecretIngest::new(
            alice2_x25519.clone(), // recipient
            alice1_x25519.clone(), // sender
            secret_exp,
            Some(key_ref.clone()),
        );
        let key_ref2: XSalsa20Poly1305KeyRef = conductor2
            .call(&alice2, "x_salsa20_poly1305_shared_secret_ingest", ing_input)
            .await;
        assert_eq!(key_ref, key_ref2);

        // now decrypt the message on conductor2
        let dec_input = holochain_zome_types::x_salsa20_poly1305::XSalsa20Poly1305Decrypt::new(
            key_ref,
            cipher.clone(),
        );
        let output: Option<XSalsa20Poly1305Data> = conductor2
            .call(&alice2, "x_salsa20_poly1305_decrypt", dec_input)
            .await;
        assert_eq!(&output, &Some(data));

        // -- now make sure not every key_ref can decrypt it -- //

        // create a new random shared key
        let key_ref_bad: XSalsa20Poly1305KeyRef = conductor2
            .call(
                &alice2,
                "x_salsa20_poly1305_shared_secret_create_random",
                <Option<XSalsa20Poly1305KeyRef>>::None,
            )
            .await;

        // try decrypting with key_ref_bad
        let dec_input_bad = holochain_zome_types::x_salsa20_poly1305::XSalsa20Poly1305Decrypt::new(
            key_ref_bad,
            cipher,
        );
        let output: Option<XSalsa20Poly1305Data> = conductor2
            .call(&alice2, "x_salsa20_poly1305_decrypt", dec_input_bad)
            .await;
        assert_eq!(&output, &None);
    }
}
