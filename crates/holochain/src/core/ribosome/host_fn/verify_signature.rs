use crate::core::ribosome::CallContext;
use crate::core::ribosome::HostFnAccess;
use crate::core::ribosome::RibosomeError;
use crate::core::ribosome::RibosomeT;
use holochain_keystore::AgentPubKeyExt;
use holochain_types::prelude::*;
use holochain_wasmer_host::prelude::*;
use std::sync::Arc;
use wasmer::RuntimeError;

pub fn verify_signature(
    _ribosome: Arc<impl RibosomeT>,
    call_context: Arc<CallContext>,
    input: VerifySignature,
) -> Result<bool, RuntimeError> {
    match HostFnAccess::from(&call_context.host_context()) {
        HostFnAccess {
            keystore_deterministic: Permission::Allow,
            ..
        } => tokio_helper::block_forever_on(async move {
            let VerifySignature {
                key,
                signature,
                data,
            } = input;
            key.verify_signature_raw(&signature, data.into())
                .await
                .map_err(|e| wasmer::RuntimeError::user(Box::new(e)))
        }),
        _ => Err(wasm_error!(WasmErrorInner::Host(
            RibosomeError::HostFnPermissions(
                call_context.zome.zome_name().clone(),
                call_context.function_name().clone(),
                "verify_signature".into(),
            )
            .to_string(),
        ))
        .into()),
    }
}

#[cfg(test)]
#[cfg(feature = "slow_tests")]
pub mod wasm_test {
    use crate::{
        core::ribosome::wasm_test::RibosomeTestFixture,
        sweettest::{SweetConductor, SweetZome},
    };
    use hdk::prelude::*;
    use holochain_wasm_test_utils::TestWasm;

    macro_rules! twice {
        ($e:expr) => {
            $e;
            $e;
        };
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn ribosome_verify_signature_raw_test() {
        holochain_trace::test_run();
        let RibosomeTestFixture {
            conductor,
            alice,
            alice_pubkey,
            bob,
            bob_pubkey,
            ..
        } = RibosomeTestFixture::new(TestWasm::Sign).await;

        // signatures should not change for a given pubkey
        let data = std::sync::Arc::new([1, 2, 3]);

        let alice_sig = conductor
            .keystore()
            .sign(alice_pubkey.clone(), data.clone())
            .await
            .unwrap();
        let bob_sig = conductor
            .keystore()
            .sign(bob_pubkey.clone(), data.clone())
            .await
            .unwrap();
        let mut alice_sig_bad = alice_sig.clone();
        let mut bob_sig_bad = bob_sig.clone();
        alice_sig_bad.0[0] = alice_sig_bad.0[0].wrapping_add(1);
        bob_sig_bad.0[63] = bob_sig_bad.0[63].wrapping_sub(1);

        async fn check_sig(
            conductor: &SweetConductor,
            data: &std::sync::Arc<[u8; 3]>,
            zome: &SweetZome,
            agent: &AgentPubKey,
            sig: &Signature,
        ) -> bool {
            conductor
                .call(
                    zome,
                    "verify_signature_raw",
                    VerifySignature::new_raw(agent.clone(), sig.clone(), data.clone().to_vec()),
                )
                .await
        }

        twice!(assert!(
            check_sig(&conductor, &data, &alice, &alice_pubkey, &alice_sig).await
        ));
        twice!(assert!(
            !check_sig(&conductor, &data, &alice, &alice_pubkey, &alice_sig_bad).await
        ));
        twice!(assert!(
            check_sig(&conductor, &data, &bob, &bob_pubkey, &bob_sig).await
        ));
        twice!(assert!(
            !check_sig(&conductor, &data, &bob, &bob_pubkey, &bob_sig_bad).await
        ));

        twice!(assert!(
            !check_sig(&conductor, &data, &bob, &bob_pubkey, &alice_sig).await
        ));
        twice!(assert!(
            !check_sig(&conductor, &data, &alice, &alice_pubkey, &bob_sig).await
        ));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn ribosome_verify_signature_test() {
        holochain_trace::test_run();
        let RibosomeTestFixture {
            conductor,
            alice,
            alice_pubkey,
            ..
        } = RibosomeTestFixture::new(TestWasm::Sign).await;

        let _nothing: () = conductor
            .call(&alice, "verify_signature", alice_pubkey)
            .await;
    }
}
