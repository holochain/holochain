use crate::core::ribosome::CallContext;
use crate::core::ribosome::HostFnAccess;
use crate::core::ribosome::RibosomeError;
use crate::core::ribosome::RibosomeT;
use holochain_types::prelude::*;
use holochain_wasmer_host::prelude::WasmError;
use ring::rand::SecureRandom;
use std::convert::TryInto;
use std::sync::Arc;
use xsalsa20poly1305::aead::{generic_array::GenericArray, Aead, NewAead};
use xsalsa20poly1305::XSalsa20Poly1305;

pub fn x_salsa20_poly1305_encrypt(
    _ribosome: Arc<impl RibosomeT>,
    call_context: Arc<CallContext>,
    input: XSalsa20Poly1305Encrypt,
) -> Result<XSalsa20Poly1305EncryptedData, WasmError> {
    match HostFnAccess::from(&call_context.host_context()) {
        HostFnAccess {
            keystore: Permission::Allow,
            ..
        } => {
            let system_random = ring::rand::SystemRandom::new();
            let mut nonce_bytes = [0; holochain_zome_types::x_salsa20_poly1305::nonce::NONCE_BYTES];
            system_random
                .fill(&mut nonce_bytes)
                .map_err(|ring_unspecified| wasm_error!(WasmErrorInner::Host(ring_unspecified.to_string())))?;

            // @todo use the real libsodium somehow instead of this rust crate.
            // The main issue here is dependency management - it's not necessarily simple to get libsodium
            // reliably on consumer devices, e.g. we might want to statically link it somewhere.
            // @todo this key ref should be an opaque ref to lair and the encrypt should happen in lair.
            let lib_key = GenericArray::from_slice(input.as_key_ref_ref().as_ref());
            let cipher = XSalsa20Poly1305::new(lib_key);
            let lib_nonce = GenericArray::from_slice(&nonce_bytes);
            let lib_encrypted_data = cipher
                .encrypt(lib_nonce, input.as_data_ref().as_ref())
                .map_err(|aead_error| wasm_error!(WasmErrorInner::Host(aead_error.to_string())))?;

            Ok(
                holochain_zome_types::x_salsa20_poly1305::encrypted_data::XSalsa20Poly1305EncryptedData::new(
                    match lib_nonce.as_slice().try_into() {
                        Ok(nonce) => nonce,
                        Err(secure_primitive_error) => return Err(wasm_error!(WasmErrorInner::Host(secure_primitive_error.to_string()))),
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
        ))),
    }
}

#[cfg(test)]
#[cfg(feature = "slow_tests")]
pub mod wasm_test {

    use hdk::prelude::*;
    use holochain_wasm_test_utils::TestWasm;
    use crate::core::ribosome::wasm_test::RibosomeTestFixture;

    #[tokio::test(flavor = "multi_thread")]
    #[cfg(feature = "test_utils")]
    async fn invoke_import_xsalsa20_poly1305_encrypt_test() {
        observability::test_run().ok();
        let RibosomeTestFixture {
            conductor, alice, ..
        } = RibosomeTestFixture::new(TestWasm::XSalsa20Poly1305).await;

        let key_ref = XSalsa20Poly1305KeyRef::from(
            [1; holochain_zome_types::x_salsa20_poly1305::key_ref::KEY_REF_BYTES],
        );
        let data = XSalsa20Poly1305Data::from(vec![1, 2, 3, 4]);
        let input = holochain_zome_types::x_salsa20_poly1305::XSalsa20Poly1305Encrypt::new(
            key_ref,
            data.clone(),
        );
        let output: XSalsa20Poly1305EncryptedData = conductor
            .call(&alice, "x_salsa20_poly1305_encrypt", input)
            .await;

        let decrypt_output: Option<XSalsa20Poly1305Data> = conductor
            .call(
                &alice,
                "x_salsa20_poly1305_decrypt",
                holochain_zome_types::x_salsa20_poly1305::XSalsa20Poly1305Decrypt::new(
                    key_ref,
                    output.clone(),
                ),
            )
            .await;
        assert_eq!(&decrypt_output, &Some(data),);

        let bad_key_ref = XSalsa20Poly1305KeyRef::from([2; 32]);
        let bad_output: Option<XSalsa20Poly1305Data> = conductor
            .call(
                &alice,
                "x_salsa20_poly1305_decrypt",
                holochain_zome_types::x_salsa20_poly1305::XSalsa20Poly1305Decrypt::new(
                    bad_key_ref,
                    output,
                ),
            )
            .await;
        assert_eq!(None, bad_output);
    }
}
