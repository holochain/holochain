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
