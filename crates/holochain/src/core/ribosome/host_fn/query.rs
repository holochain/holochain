use crate::core::ribosome::CallContext;
use crate::core::ribosome::HostFnAccess;
use crate::core::ribosome::RibosomeT;
use holochain_types::prelude::*;
use holochain_wasmer_host::prelude::WasmError;
use std::sync::Arc;
use crate::core::ribosome::RibosomeError;

pub fn query(
    _ribosome: Arc<impl RibosomeT>,
    call_context: Arc<CallContext>,
    input: ChainQueryFilter,
) -> Result<Vec<Element>, WasmError> {
    match HostFnAccess::from(&call_context.host_context()) {
        HostFnAccess {
            read_workspace: Permission::Allow,
            ..
        } => tokio_helper::block_forever_on(async move {
            let elements: Vec<Element> = call_context
                .host_context
                .workspace()
                .source_chain()
                .as_ref()
                .expect("Must have source chain to query the source chain")
                .query(input)
                .await
                .map_err(|source_chain_error| WasmError::Host(source_chain_error.to_string()))?;
            Ok(elements)
        }),
        _ => Err(WasmError::Host(RibosomeError::HostFnPermissions(
            call_context.zome.zome_name().clone(),
            call_context.function_name().clone(),
            "query".into()
        ).to_string()))
    }
}

#[cfg(test)]
#[cfg(feature = "slow_tests")]
pub mod slow_tests {
    use crate::fixt::ZomeCallHostAccessFixturator;
    use ::fixt::prelude::*;
    use hdk::prelude::*;
    use query::ChainQueryFilter;

    use holochain_wasm_test_utils::TestWasm;

    #[tokio::test(flavor = "multi_thread")]
    async fn query_smoke_test() {
        let host_access = fixt!(ZomeCallHostAccess, Predictable);

        let _hash_a: EntryHash =
            crate::call_test_ribosome!(host_access, TestWasm::Query, "add_path", "a".to_string())
                .unwrap();
        let _hash_b: EntryHash =
            crate::call_test_ribosome!(host_access, TestWasm::Query, "add_path", "b".to_string())
                .unwrap();

        let elements: Vec<Element> = crate::call_test_ribosome!(
            host_access,
            TestWasm::Query,
            "query",
            ChainQueryFilter::default()
        )
        .unwrap();

        assert_eq!(elements.len(), 5);
    }
}
