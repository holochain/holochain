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

    #[tokio::test(flavor = "multi_thread")]
    async fn invoke_import_zome_info_test() {
        let host_access = fixt!(ZomeCallHostAccess, Predictable);
        let zome_info: ZomeInfo =
            crate::call_test_ribosome!(host_access, TestWasm::ZomeInfo, "zome_info", ()).unwrap();
        assert_eq!(zome_info.name, "zome_info".into());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn zome_info() {
        let host_access = fixt!(ZomeCallHostAccess, Predictable);
        let zome_info: ZomeInfo = crate::call_test_ribosome!(host_access, TestWasm::EntryDefs, "zome_info", ()).unwrap();
        assert_eq!(
            zome_info.name,
            ZomeName::new("entry_defs"),
        );
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
            zome_info.externs,
            vec![
                Extern::new(FunctionName::new("__allocate"), true),
                Extern::new(FunctionName::new("__data_end"), true),
                Extern::new(FunctionName::new("__deallocate"), true),
                Extern::new(FunctionName::new("__heap_base"), true),
                Extern::new(FunctionName::new("assert_indexes"), true),
                Extern::new(FunctionName::new("entry_defs"), true),
                Extern::new(FunctionName::new("memory"), true),
                Extern::new(FunctionName::new("zome_info"), true),
            ],
        );

        crate::call_test_ribosome!(host_access, TestWasm::EntryDefs, "set_access", ()).unwrap();
        let zome_info_2: ZomeInfo = crate::call_test_ribosome!(host_access, TestWasm::EntryDefs, "zome_info", ()).unwrap();
        assert_eq!(
            zome_info_2.externs,
            vec![
                Extern::new(FunctionName::new("__allocate"), true),
                Extern::new(FunctionName::new("__data_end"), true),
                Extern::new(FunctionName::new("__deallocate"), true),
                Extern::new(FunctionName::new("__heap_base"), true),
                Extern::new(FunctionName::new("assert_indexes"), true),
                Extern::new(FunctionName::new("entry_defs"), true),
                Extern::new(FunctionName::new("memory"), true),
                Extern::new(FunctionName::new("zome_info"), true),
            ],
        );
    }
}
