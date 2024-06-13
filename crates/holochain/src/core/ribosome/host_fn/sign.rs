use crate::core::ribosome::CallContext;
use crate::core::ribosome::HostFnAccess;
use crate::core::ribosome::RibosomeError;
use crate::core::ribosome::RibosomeT;
use holochain_types::prelude::*;
use holochain_wasmer_host::prelude::*;
use std::sync::Arc;
use wasmer::RuntimeError;

#[tracing::instrument(skip(_ribosome, call_context))]
pub fn sign(
    _ribosome: Arc<impl RibosomeT>,
    call_context: Arc<CallContext>,
    input: Sign,
) -> Result<Signature, RuntimeError> {
    match HostFnAccess::from(&call_context.host_context()) {
        HostFnAccess {
            keystore: Permission::Allow,
            ..
        } => tokio_helper::block_forever_on(async move {
            call_context
                .host_context
                .keystore()
                .sign(input.key, input.data.into_vec().into())
                .await
        })
        .map_err(|keystore_error| -> RuntimeError {
            wasm_error!(WasmErrorInner::Host(keystore_error.to_string())).into()
        }),
        _ => Err(wasm_error!(WasmErrorInner::Host(
            RibosomeError::HostFnPermissions(
                call_context.zome.zome_name().clone(),
                call_context.function_name().clone(),
                "sign".into(),
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
    async fn ribosome_sign_test() {
        holochain_trace::test_run();
        let RibosomeTestFixture {
            conductor,
            alice,
            alice_pubkey,
            bob_pubkey,
            ..
        } = RibosomeTestFixture::new(TestWasm::Sign).await;

        for (key, data) in [
            (alice_pubkey.clone(), vec![100_u8, 200_u8, 50_u8]),
            (bob_pubkey.clone(), vec![100_u8, 200_u8, 50_u8]),
            (alice_pubkey, vec![1_u8, 2_u8, 3_u8]),
            (bob_pubkey, vec![1_u8, 2_u8, 3_u8]),
        ] {
            let mut sigs = HashSet::new();
            for _ in 0..2 {
                let signature: Signature = conductor
                    .call(&alice, "sign", Sign::new_raw(key.clone(), data.clone()))
                    .await;

                sigs.insert(signature.clone());

                let valid: bool = conductor
                    .call(
                        &alice,
                        "verify_signature_raw",
                        VerifySignature {
                            key: key.clone(),
                            signature,
                            data: data.clone(),
                        },
                    )
                    .await;

                assert!(valid);
            }
            assert_eq!(sigs.len(), 1);
        }
    }
}
