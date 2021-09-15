use crate::core::ribosome::CallContext;
use crate::core::ribosome::RibosomeT;
use std::sync::Arc;
use holochain_wasmer_host::prelude::WasmError;
use holochain_types::prelude::*;

pub fn schedule(
    _ribosome: Arc<impl RibosomeT>,
    call_context: Arc<CallContext>,
    input: String,
) -> Result<(), WasmError> {
    match HostFnAccess::from(&call_context.host_context()) {
        HostFnAccess{ write_workspace: Permission::Allow, .. } => {
            call_context.host_context().workspace().source_chain().scratch().apply(|scratch| {
                scratch.add_scheduled_fn(ScheduledFn::new(call_context.zome.zome_name().clone(), input.into()));
            }).map_err(|e| WasmError::Host(e.to_string()))?;
            Ok(())
        },
        _ => unreachable!(),
    }
}

#[cfg(test)]
pub mod tests {
    use ::fixt::prelude::*;
    use holochain_wasm_test_utils::TestWasm;
    use crate::sweettest::SweetDnaFile;
    use holochain_types::prelude::AgentPubKeyFixturator;
    use crate::core::ribosome::MockDnaStore;
    use hdk::prelude::*;
    use crate::sweettest::SweetConductor;
    use crate::conductor::ConductorBuilder;

    #[tokio::test(flavor = "multi_thread")]
    #[cfg(feature = "test_utils")]
    async fn schedule_test() -> anyhow::Result<()> {
        observability::test_run().ok();
        let (dna_file, _) = SweetDnaFile::unique_from_test_wasms(vec![TestWasm::Schedule])
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
        let alice = alice.zome(TestWasm::Schedule);
        let _bobbo = bobbo.zome(TestWasm::Schedule);

        let schedule: () = conductor
            .call(
                &alice,
                "schedule",
                ()
            )
            .await;
        dbg!(&schedule);
        let mut i: usize = 0;
        while i < 6 {
            tokio::time::sleep(std::time::Duration::from_millis(2)).await;
            conductor.handle().dispatch_scheduled_fns().await;
            let query: Vec<Element> = conductor
                .call(
                    &alice,
                    "query",
                    ()
                )
                .await;
            dbg!(&query);
            i = i + 1;
        }
        Ok(())
    }
}