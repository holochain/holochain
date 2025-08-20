use crate::core::ribosome::CallContext;
use crate::core::ribosome::HostFnAccess;
use crate::core::ribosome::RibosomeError;
use crate::core::ribosome::RibosomeT;
use holochain_types::prelude::*;
use holochain_wasmer_host::prelude::*;
use std::sync::Arc;
use wasmer::RuntimeError;

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
            let mut pk = [0; sodoken::sign::PUBLICKEYBYTES];
            let mut sk = sodoken::SizedLockedArray::<{ sodoken::sign::SECRETKEYBYTES }>::new()?;
            sodoken::sign::keypair(&mut pk, &mut sk.lock())?;

            let mut signatures = Vec::new();

            let mut sig = [0; sodoken::sign::SIGNATUREBYTES];
            for data in input.into_inner().into_iter() {
                sodoken::sign::sign_detached(&mut sig, &data, &sk.lock())?;
                signatures.push(sig.into());
            }

            std::io::Result::Ok(EphemeralSignatures {
                signatures,
                key: AgentPubKey::from_raw_32(pk.to_vec()),
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
    use crate::test_utils::RibosomeTestFixture;
    use hdk::prelude::*;
    use holochain_keystore::AgentPubKeyExt;
    use holochain_wasm_test_utils::TestWasm;

    #[tokio::test(flavor = "multi_thread")]
    async fn ribosome_sign_ephemeral_test() {
        holochain_trace::test_run();
        let RibosomeTestFixture {
            conductor, alice, ..
        } = RibosomeTestFixture::new(TestWasm::Sign).await;

        let output: Vec<EphemeralSignatures> = conductor.call(&alice, "sign_ephemeral", ()).await;

        #[derive(Serialize, Deserialize, Debug)]
        struct One([u8; 2]);
        #[derive(Serialize, Deserialize, Debug)]
        struct Two([u8; 2]);

        assert!(output[0]
            .key
            .verify_signature_raw(
                &output[0].signatures[0],
                holochain_serialized_bytes::encode(&One([1, 2]))
                    .unwrap()
                    .into()
            )
            .await
            .unwrap());

        assert!(output[0]
            .key
            .verify_signature_raw(
                &output[0].signatures[1],
                holochain_serialized_bytes::encode(&One([3, 4]))
                    .unwrap()
                    .into()
            )
            .await
            .unwrap());

        assert!(output[1]
            .key
            .verify_signature_raw(
                &output[1].signatures[0],
                holochain_serialized_bytes::encode(&One([1, 2]))
                    .unwrap()
                    .into()
            )
            .await
            .unwrap());

        assert!(output[1]
            .key
            .verify_signature_raw(
                &output[1].signatures[1],
                holochain_serialized_bytes::encode(&Two([2, 3]))
                    .unwrap()
                    .into()
            )
            .await
            .unwrap());
    }
}
