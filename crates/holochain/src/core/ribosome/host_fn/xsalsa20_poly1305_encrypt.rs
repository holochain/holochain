use crate::core::ribosome::error::RibosomeResult;
use crate::core::ribosome::CallContext;
use crate::core::ribosome::RibosomeT;
use holochain_zome_types::xsalsa20_poly1305::encrypted_data::XSalsa20Poly1305EncryptedData;
use holochain_zome_types::XSalsa20Poly1305EncryptInput;
use holochain_zome_types::XSalsa20Poly1305EncryptOutput;
use std::sync::Arc;
use xsalsa20poly1305::XSalsa20Poly1305;
use xsalsa20poly1305::aead::{ Aead, NewAead, generic_array::GenericArray };

pub fn xsalsa20_poly1305_encrypt(
    _ribosome: Arc<impl RibosomeT>,
    _call_context: Arc<CallContext>,
    input: XSalsa20Poly1305EncryptInput,
) -> RibosomeResult<XSalsa20Poly1305EncryptOutput> {
    let (key, nonce, data) = input.into_inner();

    let lib_key = GenericArray::from_slice(key.as_ref());
    let cipher = XSalsa20Poly1305::new(lib_key);
    let lib_nonce = GenericArray::from_slice(nonce.as_ref());
    let lib_encrypted_data = cipher.encrypt(lib_nonce, data.as_ref()).unwrap();
    dbg!(key);
    dbg!(nonce);
    dbg!(data);
    dbg!(lib_encrypted_data);
    Ok(XSalsa20Poly1305EncryptOutput::new(
        XSalsa20Poly1305EncryptedData::from(vec![]),
    ))
}

#[cfg(test)]
#[cfg(feature = "slow_tests")]
pub mod wasm_test {

    use crate::fixt::ZomeCallHostAccessFixturator;
    use ::fixt::prelude::*;
    use holochain_wasm_test_utils::TestWasm;
    use hdk3::prelude::*;

    #[tokio::test(threaded_scheduler)]
    async fn invoke_import_secret_box_test() {
        let test_env = holochain_state::test_utils::test_cell_env();
        let env = test_env.env();
        let mut workspace =
            crate::core::workflow::CallZomeWorkspace::new(env.clone().into()).unwrap();
        crate::core::workflow::fake_genesis(&mut workspace.source_chain)
            .await
            .unwrap();

        let workspace_lock = crate::core::workflow::CallZomeWorkspaceLock::new(workspace);

        let mut host_access = fixt!(ZomeCallHostAccess);
        host_access.workspace = workspace_lock;
        let key = XSalsa20Poly1305Key::from([1; 32]);
        let nonce = XSalsa20Poly1305Nonce::from([0; 24]);
        let data = XSalsa20Poly1305Data::from(vec![1, 2, 3, 4]);
        let input = XSalsa20Poly1305EncryptInput::new(
            (key, nonce, data)
        );
        let _: SecretBoxOutput =
            crate::call_test_ribosome!(host_access, TestWasm::XSalsa20Poly1305, "xsalsa20_poly1305_encrypt", input);
    }
}
