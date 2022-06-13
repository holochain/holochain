use crate::core::ribosome::CallContext;
use crate::core::ribosome::RibosomeT;
use holochain_types::prelude::*;
use holochain_wasmer_host::prelude::*;
use std::sync::Arc;
use crate::core::ribosome::HostFnAccess;
use crate::core::ribosome::error::RibosomeError;

pub fn zome_info(
    ribosome: Arc<impl RibosomeT>,
    call_context: Arc<CallContext>,
    _input: (),
) -> Result<ZomeInfo, RuntimeError> {
    match HostFnAccess::from(&call_context.host_context()) {
        HostFnAccess{ bindings_deterministic: Permission::Allow, .. } => {
            ribosome.zome_info(call_context.zome.clone()).map_err(|e| match e {
                RibosomeError::WasmRuntimeError(wasm_error) => wasm_error,
                other_error => wasm_error!(WasmErrorInner::Host(other_error.to_string())).into(),
            })
        },
        _ => Err(wasm_error!(WasmErrorInner::Host(RibosomeError::HostFnPermissions(
            call_context.zome.zome_name().clone(),
            call_context.function_name().clone(),
            "zome_info".into()
        ).to_string())).into())
    }
}

#[cfg(test)]
#[cfg(feature = "slow_tests")]
pub mod test {
    use holochain_wasm_test_utils::TestWasm;
    use holochain_zome_types::prelude::*;
    use crate::core::ribosome::wasm_test::RibosomeTestFixture;

    #[tokio::test(flavor = "multi_thread")]
    async fn zome_info_test() {
        observability::test_run().ok();
        let RibosomeTestFixture {
            conductor, alice, ..
        } = RibosomeTestFixture::new(TestWasm::EntryDefs).await;

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
                    required_validations: Default::default(),
                    required_validation_type: Default::default(),
                },
                EntryDef {
                    id: "comment".into(),
                    visibility: EntryVisibility::Private,
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
