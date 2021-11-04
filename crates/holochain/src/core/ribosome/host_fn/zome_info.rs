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
            ribosome.zome_info(call_context.host_context.clone(), call_context.zome.clone()).map_err(|e| match e {
                RibosomeError::WasmError(wasm_error) => wasm_error,
                other_error => WasmError::Host(other_error.to_string()),
            })
        },
        _ => unreachable!(),
    }
}

#[cfg(test)]
#[cfg(feature = "slow_tests")]
pub mod test {
    use crate::{conductor::ConductorBuilder, fixt::ZomeCallHostAccessFixturator, sweettest::{SweetConductor, SweetDnaFile}};
    use ::fixt::prelude::*;
    use holochain_types::prelude::MockDnaStore;
    use holochain_wasm_test_utils::TestWasm;
    use holochain_zome_types::prelude::*;

    #[tokio::test(flavor = "multi_thread")]
    async fn invoke_import_zome_info_test() {
        let host_access = fixt!(ZomeCallHostAccess, Predictable);
        let zome_info: ZomeInfo =
            crate::call_test_ribosome!(host_access, TestWasm::ZomeInfo, "zome_info", ()).unwrap();
        assert_eq!(zome_info.name, "zome_info".into());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn zome_info() {
        observability::test_run().ok();
        let (dna_file, _) = SweetDnaFile::unique_from_test_wasms(vec![TestWasm::ZomeInfo, TestWasm::EntryDefs])
        .await
        .unwrap();

        let alice_pubkey = fixt!(AgentPubKey, Predictable, 0);

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
                &[alice_pubkey.clone()],
                &[dna_file.into()],
            )
            .await
            .unwrap();

        let ((alice,),) = apps.into_tuples();
        let alice_zome_info = alice.zome(TestWasm::ZomeInfo);
        let alice_entry_defs = alice.zome(TestWasm::EntryDefs);

        let zome_info_entry_defs: ZomeInfo = conductor.call(&alice_entry_defs, "zome_info", ()).await;

        assert_eq!(
            zome_info_entry_defs.name,
            ZomeName::new("entry_defs"),
        );
        assert_eq!(
            zome_info_entry_defs.id,
            ZomeId::new(1)
        );
        assert_eq!(
            zome_info_entry_defs.entry_defs,
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

        let zome_info: ZomeInfo = conductor.call(&alice_zome_info, "zome_info", ()).await;
        assert_eq!(
            zome_info.externs,
            vec![
                Extern::new(FunctionName::new("__allocate"), true),
                Extern::new(FunctionName::new("__data_end"), true),
                Extern::new(FunctionName::new("__deallocate"), true),
                Extern::new(FunctionName::new("__heap_base"), true),
                Extern::new(FunctionName::new("call_info"), true),
                Extern::new(FunctionName::new("dna_info"), true),
                Extern::new(FunctionName::new("entry_defs"), true),
                Extern::new(FunctionName::new("memory"), true),
                Extern::new(FunctionName::new("remote_call_info"), true),
                Extern::new(FunctionName::new("remote_remote_call_info"), true),
                Extern::new(FunctionName::new("set_access"), true),
                Extern::new(FunctionName::new("zome_info"), true),
            ],
        );

        let _: () = conductor.call(&alice_zome_info, "set_access", ()).await;
        let zome_info_2: ZomeInfo = conductor.call(&alice_zome_info, "zome_info", ()).await;

        assert_eq!(
            zome_info_2.externs,
            vec![
                Extern::new(FunctionName::new("__allocate"), true),
                Extern::new(FunctionName::new("__data_end"), true),
                Extern::new(FunctionName::new("__deallocate"), true),
                Extern::new(FunctionName::new("__heap_base"), true),
                Extern::new(FunctionName::new("call_info"), false),
                Extern::new(FunctionName::new("dna_info"), true),
                Extern::new(FunctionName::new("entry_defs"), true),
                Extern::new(FunctionName::new("memory"), true),
                Extern::new(FunctionName::new("remote_call_info"), false),
                Extern::new(FunctionName::new("remote_remote_call_info"), true),
                Extern::new(FunctionName::new("set_access"), true),
                Extern::new(FunctionName::new("zome_info"), true),
            ],
        );
    }
}
