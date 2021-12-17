use crate::core::ribosome::CallContext;
use crate::core::ribosome::RibosomeT;
use holochain_types::prelude::*;
use holochain_wasmer_host::prelude::WasmError;
use ring::rand::SecureRandom;
use ring::rand::SystemRandom;
use ring::signature::Ed25519KeyPair;
use ring::signature::KeyPair;
use std::sync::Arc;
use crate::core::ribosome::HostFnAccess;
use crate::core::ribosome::RibosomeError;

pub fn sign_ephemeral(
    _ribosome: Arc<impl RibosomeT>,
    call_context: Arc<CallContext>,
    input: SignEphemeral,
) -> Result<EphemeralSignatures, WasmError> {
    match HostFnAccess::from(&call_context.host_context()) {
        HostFnAccess{ keystore: Permission::Allow, .. } => {
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
        },
        _ => Err(WasmError::Host(RibosomeError::HostFnPermissions(
            call_context.zome.zome_name().clone(),
            call_context.function_name().clone(),
            "sign_ephemeral".into()
        ).to_string()))
    }

}

#[cfg(test)]
#[cfg(feature = "slow_tests")]
pub mod wasm_test {
    use crate::fixt::ZomeCallHostAccessFixturator;
    use ::fixt::prelude::*;
    use hdk::prelude::*;
    use holochain_keystore::AgentPubKeyExt;
    use holochain_wasm_test_utils::TestWasm;

    #[tokio::test(flavor = "multi_thread")]
    async fn ribosome_sign_ephemeral_test() {
        let host_access = fixt!(ZomeCallHostAccess, Predictable);

        let output: Vec<EphemeralSignatures> =
            crate::call_test_ribosome!(host_access, TestWasm::Sign, "sign_ephemeral", ()).unwrap();

        #[derive(Serialize, Deserialize, Debug)]
        struct One([u8; 2]);
        #[derive(Serialize, Deserialize, Debug)]
        struct Two([u8; 2]);

        assert!(output[0]
            .key
            .verify_signature_raw(
                &output[0].signatures[0],
                holochain_serialized_bytes::encode(&One([1, 2])).unwrap().into()
            )
            .await);

        assert!(output[0]
            .key
            .verify_signature_raw(
                &output[0].signatures[1],
                holochain_serialized_bytes::encode(&One([3, 4])).unwrap().into()
            )
            .await);

        assert!(output[1]
            .key
            .verify_signature_raw(
                &output[1].signatures[0],
                holochain_serialized_bytes::encode(&One([1, 2])).unwrap().into()
            )
            .await);

        assert!(output[1]
            .key
            .verify_signature_raw(
                &output[1].signatures[1],
                holochain_serialized_bytes::encode(&Two([2, 3])).unwrap().into()
            )
            .await);
    }
}
