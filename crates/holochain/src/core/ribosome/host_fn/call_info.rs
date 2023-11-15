use crate::core::ribosome::CallContext;
use crate::core::ribosome::InvocationAuth;
use crate::core::ribosome::RibosomeError;
use crate::core::ribosome::RibosomeT;
use holochain_state::source_chain::SourceChainError;
use holochain_types::prelude::*;
use holochain_wasmer_host::prelude::*;
use holochain_zome_types::info::CallInfo;
use std::sync::Arc;

pub fn call_info(
    _ribosome: Arc<impl RibosomeT>,
    call_context: Arc<CallContext>,
    _input: (),
) -> Result<CallInfo, RuntimeError> {
    match HostFnAccess::from(&call_context.host_context()) {
        HostFnAccess {
            bindings: Permission::Allow,
            ..
        } => {
            let (provenance, cap_grant) = {
                match call_context.auth() {
                    InvocationAuth::Cap(provenance, cap_secret) => {
                        let check_function = (
                            call_context.zome.zome_name().clone(),
                            call_context.function_name().clone(),
                        );
                        let check_agent = provenance.clone();
                        let call_context = call_context.clone();
                        let cap_grant = tokio_helper::block_forever_on(async move {
                            Result::<_, WasmError>::Ok(call_context
                            .host_context
                            .workspace()
                            .source_chain()
                            .as_ref()
                            .expect("Must have source chain if bindings access is given")
                            .valid_cap_grant(
                                check_function,
                                check_agent,
                                cap_secret,
                            ).await.map_err(|e| wasm_error!(WasmErrorInner::Host(e.to_string())))?
                            // This is really a problem.
                            // It means that the host function calling into `call_info`
                            // is using a cap secret that never had authorization to call in the first place.
                            // The host must NEVER allow this so `None` is a critical bug.
                            .expect("The host is using an unauthorized cap_secret, which should never happen"))
                        })?;
                        (provenance, cap_grant)
                    }
                    InvocationAuth::LocalCallback => {
                        let author = call_context
                            .host_context
                            .workspace()
                            .source_chain()
                            .as_ref()
                            .expect("Must have source chain if bindings access is given")
                            .agent_pubkey()
                            .clone();
                        (author.clone(), CapGrant::ChainAuthor(author))
                    }
                }
            };
            Ok(CallInfo {
                function_name: call_context.function_name.clone(),
                as_at: call_context
                    .host_context
                    .workspace()
                    .source_chain()
                    .as_ref()
                    .expect("Must have source chain if bindings access is given")
                    .persisted_head_info()
                    .ok_or(wasm_error!(WasmErrorInner::Host(
                        SourceChainError::ChainEmpty.to_string()
                    )))?
                    .into_tuple(),
                provenance,
                cap_grant,
            })
        }
        _ => Err(wasm_error!(WasmErrorInner::Host(
            RibosomeError::HostFnPermissions(
                call_context.zome.zome_name().clone(),
                call_context.function_name().clone(),
                "call_info".into()
            )
            .to_string()
        ))
        .into()),
    }
}

#[cfg(test)]
#[cfg(feature = "slow_tests")]
pub mod test {
    use crate::core::ribosome::wasm_test::RibosomeTestFixture;
    use holochain_wasm_test_utils::TestWasm;
    use holochain_zome_types::prelude::*;

    #[tokio::test(flavor = "multi_thread")]
    async fn call_info_test() {
        holochain_trace::test_run().ok();
        let RibosomeTestFixture {
            conductor, alice, ..
        } = RibosomeTestFixture::new(TestWasm::ZomeInfo).await;

        let call_info: CallInfo = conductor.call(&alice, "call_info", ()).await;
        assert_eq!(call_info.as_at.1, 3);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn call_info_provenance_test() {
        holochain_trace::test_run().ok();
        let RibosomeTestFixture {
            conductor,
            alice,
            alice_pubkey,
            bob,
            bob_pubkey,
            ..
        } = RibosomeTestFixture::new(TestWasm::ZomeInfo).await;

        let _: () = conductor.call(&alice, "set_access", ()).await;
        let _: () = conductor.call(&bob, "set_access", ()).await;

        let alice_call_info: CallInfo = conductor.call(&alice, "call_info", ()).await;
        let bob_call_info: CallInfo = conductor.call(&bob, "call_info", ()).await;
        let bob_call_alice_call_info: CallInfo = conductor
            .call(&bob, "remote_call_info", alice_pubkey.clone())
            .await;
        let alice_call_bob_call_alice_call_info: CallInfo = conductor
            .call(&alice, "remote_remote_call_info", bob_pubkey.clone())
            .await;

        // direct calls to alice/bob should have their own provenance
        assert_eq!(alice_call_info.provenance, alice_pubkey);
        assert_eq!(bob_call_info.provenance, bob_pubkey);
        // Bob calling into alice should have bob provenance.
        assert_eq!(bob_call_alice_call_info.provenance, bob_pubkey);
        // Alice calling back into herself via. bob should have bob provenance.
        assert_eq!(alice_call_bob_call_alice_call_info.provenance, bob_pubkey);
    }
}
