use crate::core::ribosome::CallContext;
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
    _call_context: Arc<CallContext>,
    input: XSalsa20Poly1305Encrypt,
) -> Result<XSalsa20Poly1305EncryptedData, WasmError> {
    let system_random = ring::rand::SystemRandom::new();
    let mut nonce_bytes = [0; holochain_zome_types::x_salsa20_poly1305::nonce::NONCE_BYTES];
    system_random
        .fill(&mut nonce_bytes)
        .map_err(|ring_unspecified| WasmError::Host(ring_unspecified.to_string()))?;

    // @todo use the real libsodium somehow instead of this rust crate.
    // The main issue here is dependency management - it's not necessarily simple to get libsodium
    // reliably on consumer devices, e.g. we might want to statically link it somewhere.
    // @todo this key ref should be an opaque ref to lair and the encrypt should happen in lair.
    let lib_key = GenericArray::from_slice(input.as_key_ref_ref().as_ref());
    let cipher = XSalsa20Poly1305::new(lib_key);
    let lib_nonce = GenericArray::from_slice(&nonce_bytes);
    let lib_encrypted_data = cipher
        .encrypt(lib_nonce, input.as_data_ref().as_ref())
        .map_err(|aead_error| WasmError::Host(aead_error.to_string()))?;

    Ok(
        holochain_zome_types::x_salsa20_poly1305::encrypted_data::XSalsa20Poly1305EncryptedData::new(
            match lib_nonce.as_slice().try_into() {
                Ok(nonce) => nonce,
                Err(secure_primitive_error) => return Err(WasmError::Host(secure_primitive_error.to_string())),
            },
            lib_encrypted_data,
        ),
    )
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
    async fn invoke_import_xsalsa20_poly1305_encrypt_test() {
        let test_env = holochain_state::test_utils::test_cell_env();
        let test_cache = holochain_state::test_utils::test_cache_env();
        let env = test_env.env();
        let author = fake_agent_pubkey_1();
        crate::test_utils::fake_genesis(env.clone())
            .await
            .unwrap();
        let workspace = HostFnWorkspace::new(env.clone(), test_cache.env(), author).unwrap();

        let mut host_access = fixt!(ZomeCallHostAccess);
        host_access.workspace = workspace;
        let key_ref = XSalsa20Poly1305KeyRef::from(
            [1; holochain_zome_types::x_salsa20_poly1305::key_ref::KEY_REF_BYTES],
        );
        let data = XSalsa20Poly1305Data::from(vec![1, 2, 3, 4]);
        let input = holochain_zome_types::x_salsa20_poly1305::XSalsa20Poly1305Encrypt::new(
            key_ref,
            data.clone(),
        );
        let output: XSalsa20Poly1305EncryptedData = crate::call_test_ribosome!(
            host_access,
            TestWasm::XSalsa20Poly1305,
            "x_salsa20_poly1305_encrypt",
            input
        );
        let decrypt_output: Option<XSalsa20Poly1305Data> = crate::call_test_ribosome!(
            host_access,
            TestWasm::XSalsa20Poly1305,
            "x_salsa20_poly1305_decrypt",
            holochain_zome_types::x_salsa20_poly1305::XSalsa20Poly1305Decrypt::new(
                key_ref,
                output.clone(),
            )
        );
        assert_eq!(&decrypt_output, &Some(data),);

        let bad_key_ref = XSalsa20Poly1305KeyRef::from([2; 32]);
        let bad_output: Option<XSalsa20Poly1305Data> = crate::call_test_ribosome!(
            host_access,
            TestWasm::XSalsa20Poly1305,
            "x_salsa20_poly1305_decrypt",
            holochain_zome_types::x_salsa20_poly1305::XSalsa20Poly1305Decrypt::new(
                bad_key_ref,
                output
            )
        );
        assert_eq!(None, bad_output);
    }
}
