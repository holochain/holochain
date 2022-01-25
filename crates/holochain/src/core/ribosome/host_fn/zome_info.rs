use crate::core::ribosome::CallContext;
use crate::core::ribosome::RibosomeT;
use holochain_types::prelude::*;
use holochain_wasmer_host::prelude::WasmError;
use std::sync::Arc;
use crate::core::ribosome::HostFnAccess;
use crate::core::ribosome::error::RibosomeError;

pub fn zome_info(
    ribosome: Arc<impl RibosomeT>,
    call_context: Arc<CallContext>,
    _input: (),
) -> Result<ZomeInfo, WasmError> {
    match HostFnAccess::from(&call_context.host_context()) {
        HostFnAccess{ bindings_deterministic: Permission::Allow, .. } => {
            ribosome.zome_info(call_context.zome.clone()).map_err(|e| match e {
                RibosomeError::WasmError(wasm_error) => wasm_error,
                other_error => WasmError::Host(other_error.to_string()),
            })
        },
        _ => Err(WasmError::Host(RibosomeError::HostFnPermissions(
            call_context.zome.zome_name().clone(),
            call_context.function_name().clone(),
            "zome_info".into()
        ).to_string()))
    }
}

#[cfg(test)]
#[cfg(feature = "slow_tests")]
pub mod test {
    use ::fixt::prelude::*;
    use holochain_wasm_test_utils::TestWasm;
    use holochain_zome_types::prelude::*;
    use crate::sweettest::SweetDnaFile;
    use crate::core::ribosome::MockDnaStore;
    use crate::sweettest::SweetConductor;
    use crate::conductor::ConductorBuilder;

    #[tokio::test(flavor = "multi_thread")]
    async fn zome_info_test() {
        observability::test_run().ok();
        let (dna_file, _) = SweetDnaFile::unique_from_test_wasms(vec![TestWasm::EntryDefs])
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
        let alice = alice.zome(TestWasm::EntryDefs);
        let _bobbo = bobbo.zome(TestWasm::EntryDefs);

        let zome_info: ZomeInfo = conductor.call(&alice, "zome_info", ()).await;
        assert_eq!(zome_info.name, "entry_defs".into());
        assert_eq!(
            zome_info.id,
            ZomeId::new(0)
        );
        assert_eq!(
            zome_info.entry_defs,
            vec![
                EntryDef {
                    id: "post".into(),
                    visibility: Default::default(),
                    crdt_type: Default::default(),
                    required_validations: Default::default(),
                    required_validation_type: Default::default(),
                },
                EntryDef {
                    id: "comment".into(),
                    visibility: EntryVisibility::Private,
                    crdt_type: Default::default(),
                    required_validations: Default::default(),
                    required_validation_type: Default::default(),
                }
            ].into(),
        );
        assert_eq!(
            zome_info.extern_fns,
            vec![
                FunctionName::new("__allocate"),
                FunctionName::new("__data_end"),
                FunctionName::new("__deallocate"),
                FunctionName::new("__heap_base"),
                FunctionName::new("assert_indexes"),
                FunctionName::new("entry_defs"),
                FunctionName::new("memory"),
                FunctionName::new("zome_info"),
            ],
        );
    }
}
