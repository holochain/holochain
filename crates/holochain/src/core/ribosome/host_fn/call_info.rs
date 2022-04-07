use crate::core::ribosome::CallContext;
use crate::core::ribosome::InvocationAuth;
use crate::core::ribosome::RibosomeT;
use holochain_types::prelude::*;
use holochain_wasmer_host::prelude::*;
use holochain_zome_types::info::CallInfo;
use std::sync::Arc;

pub fn call_info(
    _ribosome: Arc<impl RibosomeT>,
    call_context: Arc<CallContext>,
    _input: (),
) -> Result<CallInfo, WasmError> {
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
                    .persisted_chain_head(),
                provenance,
                cap_grant,
            })
        }
        _ => unreachable!(),
    }
}

#[cfg(test)]
#[cfg(feature = "slow_tests")]
pub mod test {
    use crate::conductor::ConductorBuilder;
    use crate::core::ribosome::MockDnaStore;
    use crate::fixt::ZomeCallHostAccessFixturator;
    use crate::sweettest::SweetConductor;
    use crate::sweettest::SweetDnaFile;
    use ::fixt::prelude::*;
    use holochain_wasm_test_utils::TestWasm;
    use holochain_zome_types::prelude::*;

    #[tokio::test(flavor = "multi_thread")]
    async fn invoke_import_call_info_test() {
        let host_access = fixt!(ZomeCallHostAccess, Predictable);
        let call_info: CallInfo =
            crate::call_test_ribosome!(host_access, TestWasm::ZomeInfo, "call_info", ()).unwrap();
        assert_eq!(call_info.as_at.1, 2);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn call_info_provenance_test() {
        observability::test_run().ok();
        let (dna_file, _) = SweetDnaFile::unique_from_test_wasms(vec![TestWasm::ZomeInfo])
            .await
            .unwrap();

        let alice_pubkey = fixt!(AgentPubKey, Predictable, 0);
        let bob_pubkey = fixt!(AgentPubKey, Predictable, 1);

        let mut dna_store = MockDnaStore::new();
        dna_store.expect_add_dnas::<Vec<_>>().return_const(());
        dna_store.expect_add_entry_defs::<Vec<_>>().return_const(());
        dna_store.expect_add_dna().return_const(());
        dna_store
            .expect_get()
            .return_const(Some(dna_file.clone().into()));
        dna_store
            .expect_get_entry_def()
            .return_const(EntryDef::default_with_id("thing"));

        let mut conductor =
            SweetConductor::from_builder(ConductorBuilder::with_mock_dna_store(dna_store)).await;

        let apps = conductor
            .setup_app_for_agents(
                "app-",
                &[alice_pubkey.clone(), bob_pubkey.clone()],
                &[dna_file.into()],
            )
            .await
            .unwrap();

        let ((alice,), (bobbo,)) = apps.into_tuples();
        let alice = alice.zome(TestWasm::ZomeInfo);
        let bobbo = bobbo.zome(TestWasm::ZomeInfo);

        let _: () = conductor.call(&alice, "set_access", ()).await;
        let _: () = conductor.call(&bobbo, "set_access", ()).await;

        let alice_call_info: CallInfo = conductor.call(&alice, "call_info", ()).await;
        let bobbo_call_info: CallInfo = conductor.call(&bobbo, "call_info", ()).await;
        let bobbo_call_alice_call_info: CallInfo = conductor
            .call(&bobbo, "remote_call_info", alice_pubkey.clone())
            .await;
        let alice_call_bobbo_call_alice_call_info: CallInfo = conductor
            .call(&alice, "remote_remote_call_info", bob_pubkey.clone())
            .await;

        // direct calls to alice/bob should have their own provenance
        assert_eq!(alice_call_info.provenance, alice_pubkey);
        assert_eq!(bobbo_call_info.provenance, bob_pubkey);
        // Bob calling into alice should have bob provenance.
        assert_eq!(bobbo_call_alice_call_info.provenance, bob_pubkey);
        // Alice calling back into herself via. bob should have bob provenance.
        assert_eq!(alice_call_bobbo_call_alice_call_info.provenance, bob_pubkey);
    }
}
