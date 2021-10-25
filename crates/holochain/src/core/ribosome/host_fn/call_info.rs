use crate::core::ribosome::CallContext;
use crate::core::ribosome::RibosomeT;
use std::sync::Arc;
use holochain_wasmer_host::prelude::WasmError;
use holochain_zome_types::info::CallInfo;
use holochain_types::prelude::*;
use crate::core::ribosome::InvocationAuth;

pub fn call_info(
    _ribosome: Arc<impl RibosomeT>,
    call_context: Arc<CallContext>,
    _input: (),
) -> Result<CallInfo, WasmError> {
    match HostFnAccess::from(&call_context.host_context()) {
        HostFnAccess{ bindings: Permission::Allow, .. } => {
            let (provenance, cap_grant) = {
                match call_context.auth() {
                    InvocationAuth::Cap(provenance, cap_secret) => {
                        let cap_grant = call_context
                            .host_context
                            .workspace()
                            .source_chain()
                            .valid_cap_grant(
                                &(call_context.zome.zome_name().clone(), call_context.function_name().clone()),
                                &provenance,
                                cap_secret.as_ref(),
                            ).map_err(|e| WasmError::Host(e.to_string()))?
                            // This is really a problem.
                            // It means that the host function calling into `call_info`
                            // is using a cap secret that never had authorization to call in the first place.
                            // The host must NEVER allow this so `None` is a critical bug.
                            .unwrap();
                        (provenance, cap_grant)
                    },
                    InvocationAuth::LocalCallback => {
                        let author = call_context.host_context.workspace().source_chain().agent_pubkey().clone();
                        (author.clone(), CapGrant::ChainAuthor(author))
                    }
                }
            };
            Ok(CallInfo {
                as_at: call_context
                    .host_context
                    .workspace()
                    .source_chain()
                    .persisted_chain_head(),
                provenance,
                cap_grant,
            })
        },
        _ => unreachable!(),
    }
}

#[cfg(test)]
#[cfg(feature = "slow_tests")]
pub mod test {
    use crate::fixt::ZomeCallHostAccessFixturator;
    use ::fixt::prelude::*;
    use holochain_wasm_test_utils::TestWasm;
    use holochain_zome_types::prelude::*;
    use crate::sweettest::SweetDnaFile;
    use crate::core::ribosome::MockDnaStore;
    use crate::sweettest::SweetConductor;
    use crate::conductor::ConductorBuilder;

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

        let mut conductor = SweetConductor::from_builder(ConductorBuilder::with_mock_dna_store(dna_store)).await;

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

        let alice_call_info: CallInfo = conductor.call(&alice, "call_info", ()).await;
        let bobbo_call_info: CallInfo = conductor.call(&bobbo, "call_info", ()).await;
        let bobbo_call_alice_call_info: CallInfo = conductor.call(&bobbo, "remote_call_info", alice_pubkey.clone()).await;

        assert_eq!(
            alice_call_info.provenance,
            alice_pubkey
        );
        assert_eq!(
            bobbo_call_info.provenance,
            bob_pubkey
        );
        assert_eq!(
            bobbo_call_alice_call_info.provenance,
            bob_pubkey
        );
    }
}
