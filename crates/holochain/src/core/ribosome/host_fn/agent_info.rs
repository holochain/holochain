use crate::core::ribosome::CallContext;
use crate::core::ribosome::HostFnAccess;
use crate::core::ribosome::RibosomeT;
use holochain_types::prelude::*;
use holochain_wasmer_host::prelude::*;
use std::sync::Arc;

#[allow(clippy::extra_unused_lifetimes)]
pub fn agent_info<'a>(
    _ribosome: Arc<impl RibosomeT>,
    call_context: Arc<CallContext>,
    _input: (),
) -> Result<AgentInfo, WasmError> {
    match HostFnAccess::from(&call_context.host_context()) {
        HostFnAccess {
            agent_info: Permission::Allow,
            ..
        } => {
            let agent_pubkey = call_context
                .host_context
                .workspace()
                .source_chain()
                .as_ref()
                .expect("Must have source chain if agent_info access is given")
                .agent_pubkey()
                .clone();
            Ok(AgentInfo {
                agent_initial_pubkey: agent_pubkey.clone(),
                agent_latest_pubkey: agent_pubkey,
                chain_head: call_context
                    .host_context
                    .workspace()
                    .source_chain()
                    .as_ref()
                    .expect("Must have source chain if agent_info access is given")
                    .chain_head()
                    .map_err(|e| wasm_error!(WasmErrorInner::Host(e.to_string())))?,
            })
        }
        _ => unreachable!(),
    }
}

#[cfg(test)]
#[cfg(feature = "slow_tests")]
pub mod test {
    use ::fixt::prelude::*;

    use crate::conductor::ConductorBuilder;
    use crate::sweettest::SweetConductor;
    use crate::sweettest::SweetDnaFile;
    use holochain_types::prelude::*;
    use holochain_types::test_utils::fake_agent_pubkey_1;
    use holochain_wasm_test_utils::TestWasm;

    #[tokio::test(flavor = "multi_thread")]
    async fn agent_info_test() {
        observability::test_run().ok();
        let (dna_file, _) = SweetDnaFile::unique_from_test_wasms(vec![TestWasm::AgentInfo])
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
        let alice = alice.zome(TestWasm::AgentInfo);
        let _bobbo = bobbo.zome(TestWasm::AgentInfo);

        let call_info: CallInfo = conductor.call(&alice, "call_info", ()).await;
        let agent_info: AgentInfo = conductor.call(&alice, "agent_info", ()).await;
        assert_eq!(agent_info.agent_initial_pubkey, fake_agent_pubkey_1(),);
        assert_eq!(agent_info.agent_latest_pubkey, fake_agent_pubkey_1(),);

        assert_eq!(agent_info.chain_head.1, call_info.as_at.1 + 1,);

        let call_info_1: CallInfo = conductor.call(&alice, "call_info", ()).await;
        let agent_info_1: AgentInfo = conductor.call(&alice, "agent_info", ()).await;
        dbg!(&agent_info_1);
        dbg!(&call_info_1);
        assert_eq!(agent_info_1.chain_head.1, call_info_1.as_at.1 + 1,);
    }
}
