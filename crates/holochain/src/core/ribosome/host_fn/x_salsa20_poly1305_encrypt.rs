use crate::core::ribosome::error::RibosomeResult;
use crate::core::ribosome::CallContext;
use crate::core::ribosome::RibosomeT;
use holochain_zome_types::XSalsa20Poly1305EncryptInput;
use holochain_zome_types::XSalsa20Poly1305EncryptOutput;
use ring::rand::SecureRandom;
use std::convert::TryInto;
use std::sync::Arc;
use xsalsa20poly1305::aead::{generic_array::GenericArray, Aead, NewAead};
use xsalsa20poly1305::XSalsa20Poly1305;

pub fn x_salsa20_poly1305_encrypt(
    _ribosome: Arc<impl RibosomeT>,
    _call_context: Arc<CallContext>,
    input: XSalsa20Poly1305EncryptInput,
) -> RibosomeResult<XSalsa20Poly1305EncryptOutput> {
    let encrypt = input.into_inner();

    let system_random = ring::rand::SystemRandom::new();
    let mut nonce_bytes = [0; holochain_zome_types::x_salsa20_poly1305::nonce::NONCE_BYTES];
    system_random.fill(&mut nonce_bytes)?;

    // @todo use the real libsodium somehow instead of this rust crate.
    // The main issue here is dependency management - it's not necessarily simple to get libsodium
    // reliably on consumer devices, e.g. we might want to statically link it somewhere.
    // @todo this key ref should be an opaque ref to lair and the encrypt should happen in lair.
    let lib_key = GenericArray::from_slice(encrypt.as_key_ref_ref().as_ref());
    let cipher = XSalsa20Poly1305::new(lib_key);
    let lib_nonce = GenericArray::from_slice(&nonce_bytes);
    let lib_encrypted_data = cipher.encrypt(lib_nonce, encrypt.as_data_ref().as_ref())?;

    Ok(XSalsa20Poly1305EncryptOutput::new(
        holochain_zome_types::x_salsa20_poly1305::encrypted_data::XSalsa20Poly1305EncryptedData::new(
            lib_nonce.as_slice().try_into()?,
            lib_encrypted_data,
        ),
    ))
}

#[cfg(test)]
#[cfg(feature = "slow_tests")]
pub mod wasm_test {

    use crate::fixt::ZomeCallHostAccessFixturator;
    use ::fixt::prelude::*;
    use hdk3::prelude::*;
    use holochain_wasm_test_utils::TestWasm;

    #[tokio::test(threaded_scheduler)]
    async fn invoke_import_xsalsa20_poly1305_encrypt_test() {
        let test_env = holochain_lmdb::test_utils::test_cell_env();
        let env = test_env.env();
        let mut workspace =
            crate::core::workflow::CallZomeWorkspace::new(env.clone().into()).unwrap();
        crate::core::workflow::fake_genesis(&mut workspace.source_chain)
            .await
            .unwrap();

        let workspace_lock = crate::core::workflow::CallZomeWorkspaceLock::new(workspace);

        let mut host_access = fixt!(ZomeCallHostAccess);
        host_access.workspace = workspace_lock;
        let key_ref = XSalsa20Poly1305KeyRef::from(
            [1; holochain_zome_types::x_salsa20_poly1305::key_ref::KEY_REF_BYTES],
        );
        let data = XSalsa20Poly1305Data::from(vec![1, 2, 3, 4]);
        let input = XSalsa20Poly1305EncryptInput::new(
            holochain_zome_types::x_salsa20_poly1305::XSalsa20Poly1305Encrypt::new(
                key_ref,
                data.clone(),
            ),
        );
        let output: XSalsa20Poly1305EncryptOutput = crate::call_test_ribosome!(
            host_access,
            TestWasm::XSalsa20Poly1305,
            "x_salsa20_poly1305_encrypt",
            input
        );
        let decrypt_output: XSalsa20Poly1305DecryptOutput = crate::call_test_ribosome!(
            host_access,
            TestWasm::XSalsa20Poly1305,
            "x_salsa20_poly1305_decrypt",
            XSalsa20Poly1305DecryptInput::new(
                holochain_zome_types::x_salsa20_poly1305::XSalsa20Poly1305Decrypt::new(
                    key_ref,
                    output.clone().into_inner()
                )
            )
        );
        assert_eq!(&decrypt_output.clone().into_inner(), &Some(data),);

        let bad_key_ref = XSalsa20Poly1305KeyRef::from([2; 32]);
        let bad_output: XSalsa20Poly1305DecryptOutput = crate::call_test_ribosome!(
            host_access,
            TestWasm::XSalsa20Poly1305,
            "x_salsa20_poly1305_decrypt",
            XSalsa20Poly1305DecryptInput::new(
                holochain_zome_types::x_salsa20_poly1305::XSalsa20Poly1305Decrypt::new(
                    bad_key_ref,
                    output.into_inner()
                )
            )
        );
        assert_eq!(None, bad_output.into_inner(),);
    }
}
