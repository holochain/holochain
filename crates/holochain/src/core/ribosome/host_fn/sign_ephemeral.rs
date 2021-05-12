use crate::core::ribosome::CallContext;
use crate::core::ribosome::RibosomeT;
use holochain_types::prelude::*;
use holochain_wasmer_host::prelude::WasmError;
use ring::rand::SecureRandom;
use ring::rand::SystemRandom;
use ring::signature::Ed25519KeyPair;
use ring::signature::KeyPair;
use std::sync::Arc;

pub fn sign_ephemeral(
    _ribosome: Arc<impl RibosomeT>,
    _call_context: Arc<CallContext>,
    input: SignEphemeral,
) -> Result<EphemeralSignatures, WasmError> {
    let rng = SystemRandom::new();
    let mut seed = [0; 32];
    rng.fill(&mut seed)
        .map_err(|e| WasmError::Guest(e.to_string()))?;
    let ephemeral_keypair =
        Ed25519KeyPair::from_seed_unchecked(&seed).map_err(|e| WasmError::Host(e.to_string()))?;

    let signatures: Result<Vec<Signature>, _> = input
        .into_inner()
        .into_iter()
        .map(|data| ephemeral_keypair.sign(&data).as_ref().try_into())
        .collect();

    Ok(EphemeralSignatures {
        signatures: signatures.map_err(|e| WasmError::Host(e.to_string()))?,
        key: AgentPubKey::from_raw_32(ephemeral_keypair.public_key().as_ref().to_vec()),
    })
}

#[cfg(test)]
#[cfg(feature = "slow_tests")]
pub mod wasm_test {
    use crate::fixt::ZomeCallHostAccessFixturator;
    use ::fixt::prelude::*;
    use hdk::prelude::*;
    use holochain_keystore::AgentPubKeyExt;
    use holochain_state::host_fn_workspace::HostFnWorkspace;
    use holochain_wasm_test_utils::TestWasm;

    #[tokio::test(flavor = "multi_thread")]
    async fn ribosome_sign_ephemeral_test() {
        let test_env = holochain_state::test_utils::test_cell_env();
        let test_cache = holochain_state::test_utils::test_cache_env();
        let env = test_env.env();
        let author = fake_agent_pubkey_1();
        crate::test_utils::fake_genesis(env.clone())
            .await
            .unwrap();
        let workspace = HostFnWorkspace::new(env.clone(), test_cache.env(), author).unwrap();

        let mut host_access = fixt!(ZomeCallHostAccess, Predictable);
        host_access.workspace = workspace;

        let output: Vec<EphemeralSignatures> =
            crate::call_test_ribosome!(host_access, TestWasm::Sign, "sign_ephemeral", ());

        #[derive(Serialize, Deserialize, Debug)]
        struct One([u8; 2]);
        #[derive(Serialize, Deserialize, Debug)]
        struct Two([u8; 2]);

        assert!(output[0]
            .key
            .verify_signature_raw(
                &output[0].signatures[0],
                &holochain_serialized_bytes::encode(&One([1, 2])).unwrap()
            )
            .await
            .unwrap());

        assert!(output[0]
            .key
            .verify_signature_raw(
                &output[0].signatures[1],
                &holochain_serialized_bytes::encode(&One([3, 4])).unwrap()
            )
            .await
            .unwrap());

        assert!(output[1]
            .key
            .verify_signature_raw(
                &output[1].signatures[0],
                &holochain_serialized_bytes::encode(&One([1, 2])).unwrap()
            )
            .await
            .unwrap());

        assert!(output[1]
            .key
            .verify_signature_raw(
                &output[1].signatures[1],
                &holochain_serialized_bytes::encode(&Two([2, 3])).unwrap()
            )
            .await
            .unwrap());
    }
}
