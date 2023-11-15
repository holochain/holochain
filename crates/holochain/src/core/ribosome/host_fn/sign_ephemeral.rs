use crate::core::ribosome::CallContext;
use crate::core::ribosome::HostFnAccess;
use crate::core::ribosome::RibosomeError;
use crate::core::ribosome::RibosomeT;
use holochain_types::prelude::*;
use holochain_wasmer_host::prelude::*;
use std::sync::Arc;

pub fn sign_ephemeral(
    _ribosome: Arc<impl RibosomeT>,
    call_context: Arc<CallContext>,
    input: SignEphemeral,
) -> Result<EphemeralSignatures, RuntimeError> {
    match HostFnAccess::from(&call_context.host_context()) {
        HostFnAccess {
            keystore: Permission::Allow,
            ..
        } => tokio_helper::block_forever_on(async move {
            let pk = sodoken::BufWriteSized::new_no_lock();
            let sk = sodoken::BufWriteSized::new_mem_locked()?;
            sodoken::sign::keypair(pk.clone(), sk.clone()).await?;
            let pk = pk.read_lock_sized().to_vec();
            let sk = sk.to_read_sized();

            let mut signatures = Vec::new();

            let sig = sodoken::BufWriteSized::new_no_lock();
            for data in input.into_inner().into_iter() {
                sodoken::sign::detached(
                    sig.clone(),
                    data.to_vec(),
                    sk.clone(),
                ).await?;
                signatures.push((*sig.read_lock_sized()).into());
            }

            sodoken::SodokenResult::Ok(EphemeralSignatures {
                signatures,
                key: AgentPubKey::from_raw_32(pk),
            })
        })
        .map_err(|error| -> RuntimeError {
            wasm_error!(WasmErrorInner::Host(error.to_string())).into()
        }),
        _ => Err(wasm_error!(WasmErrorInner::Host(
            RibosomeError::HostFnPermissions(
                call_context.zome.zome_name().clone(),
                call_context.function_name().clone(),
                "sign_ephemeral".into(),
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
    use holochain_keystore::AgentPubKeyExt;
    use holochain_wasm_test_utils::TestWasm;

    #[tokio::test(flavor = "multi_thread")]
    async fn ribosome_sign_ephemeral_test() {
        holochain_trace::test_run().ok();
        let RibosomeTestFixture {
            conductor, alice, ..
        } = RibosomeTestFixture::new(TestWasm::Sign).await;

        let output: Vec<EphemeralSignatures> = conductor.call(&alice, "sign_ephemeral", ()).await;

        #[derive(Serialize, Deserialize, Debug)]
        struct One([u8; 2]);
        #[derive(Serialize, Deserialize, Debug)]
        struct Two([u8; 2]);

        assert!(
            output[0]
                .key
                .verify_signature_raw(
                    &output[0].signatures[0],
                    holochain_serialized_bytes::encode(&One([1, 2]))
                        .unwrap()
                        .into()
                )
                .await
        );

        assert!(
            output[0]
                .key
                .verify_signature_raw(
                    &output[0].signatures[1],
                    holochain_serialized_bytes::encode(&One([3, 4]))
                        .unwrap()
                        .into()
                )
                .await
        );

        assert!(
            output[1]
                .key
                .verify_signature_raw(
                    &output[1].signatures[0],
                    holochain_serialized_bytes::encode(&One([1, 2]))
                        .unwrap()
                        .into()
                )
                .await
        );

        assert!(
            output[1]
                .key
                .verify_signature_raw(
                    &output[1].signatures[1],
                    holochain_serialized_bytes::encode(&Two([2, 3]))
                        .unwrap()
                        .into()
                )
                .await
        );
    }
}
