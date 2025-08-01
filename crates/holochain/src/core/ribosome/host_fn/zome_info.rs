use crate::core::ribosome::error::RibosomeError;
use crate::core::ribosome::CallContext;
use crate::core::ribosome::HostFnAccess;
use crate::core::ribosome::RibosomeT;
use holochain_types::prelude::*;
use holochain_wasmer_host::prelude::*;
use std::sync::Arc;
use wasmer::RuntimeError;

pub fn zome_info(
    ribosome: Arc<impl RibosomeT>,
    call_context: Arc<CallContext>,
    _input: (),
) -> Result<ZomeInfo, RuntimeError> {
    match HostFnAccess::from(&call_context.host_context()) {
        HostFnAccess {
            bindings_deterministic: Permission::Allow,
            ..
        } => {
            let f = ribosome.zome_info(call_context.zome.clone());
            tokio_helper::block_on(f, std::time::Duration::from_secs(60))
                .map_err(|_| wasm_error!("60s timeout elapsed during zome_info()"))?
                .map_err(|e| match e {
                    RibosomeError::WasmRuntimeError(wasm_error) => wasm_error,
                    other_error => {
                        wasm_error!(WasmErrorInner::Host(other_error.to_string())).into()
                    }
                })
        }
        _ => Err(wasm_error!(WasmErrorInner::Host(
            RibosomeError::HostFnPermissions(
                call_context.zome.zome_name().clone(),
                call_context.function_name().clone(),
                "zome_info".into()
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
    async fn zome_info_test() {
        holochain_trace::test_run();
        let RibosomeTestFixture {
            conductor, alice, ..
        } = RibosomeTestFixture::new(TestWasm::EntryDefs).await;

        let zome_info: ZomeInfo = conductor.call(&alice, "zome_info", ()).await;
        assert_eq!(zome_info.name, "entry_defs".into());
        assert_eq!(zome_info.id, ZomeIndex::new(1));
        assert_eq!(
            zome_info.entry_defs,
            vec![
                EntryDef {
                    id: "post".into(),
                    visibility: Default::default(),
                    required_validations: Default::default(),
                    ..Default::default()
                },
                EntryDef {
                    id: "comment".into(),
                    visibility: EntryVisibility::Private,
                    required_validations: Default::default(),
                    ..Default::default()
                }
            ]
            .into(),
        );

        let entries = vec![(ZomeIndex(0), vec![EntryDefIndex(0), EntryDefIndex(1)])];
        let links = vec![(ZomeIndex(0), vec![])];
        assert_eq!(
            zome_info.zome_types,
            ScopedZomeTypesSet {
                entries: ScopedZomeTypes(entries),
                links: ScopedZomeTypes(links),
            }
        );
    }

    #[cfg(feature = "wasmer_sys")]
    #[tokio::test(flavor = "multi_thread")]
    async fn zome_info_extern_fns_test() {
        holochain_trace::test_run();
        let RibosomeTestFixture {
            conductor, alice, ..
        } = RibosomeTestFixture::new(TestWasm::EntryDefs).await;

        let zome_info: ZomeInfo = conductor.call(&alice, "zome_info", ()).await;

        assert_eq!(
            zome_info.extern_fns,
            vec![
                FunctionName::new("__data_end"),
                FunctionName::new("__getrandom_v03_custom"),
                FunctionName::new("__hc__allocate_1"),
                FunctionName::new("__hc__deallocate_1"),
                FunctionName::new("__heap_base"),
                FunctionName::new("assert_indexes"),
                FunctionName::new("entry_defs"),
                FunctionName::new("memory"),
                FunctionName::new("wasmer_metering_points_exhausted"),
                FunctionName::new("wasmer_metering_remaining_points"),
                FunctionName::new("zome_info"),
            ],
        );
    }


    // Same test, but excluding wasmer metering extern fns
    #[cfg(feature = "wasmer_wamr")]
    #[tokio::test(flavor = "multi_thread")]
    async fn zome_info_extern_fns_test() {
        holochain_trace::test_run();
        let RibosomeTestFixture {
            conductor, alice, ..
        } = RibosomeTestFixture::new(TestWasm::EntryDefs).await;

        let zome_info: ZomeInfo = conductor.call(&alice, "zome_info", ()).await;

        assert_eq!(
            zome_info.extern_fns,
            vec![
                FunctionName::new("__data_end"),
                FunctionName::new("__getrandom_v03_custom"),
                FunctionName::new("__hc__allocate_1"),
                FunctionName::new("__hc__deallocate_1"),
                FunctionName::new("__heap_base"),
                FunctionName::new("assert_indexes"),
                FunctionName::new("entry_defs"),
                FunctionName::new("memory"),
                FunctionName::new("zome_info"),
            ],
        );
    }
}
