use crate::core::ribosome::CallContext;
use crate::core::ribosome::RibosomeT;
use holochain_types::prelude::*;
use holochain_wasmer_host::prelude::WasmError;
use std::sync::Arc;

pub fn query(
    _ribosome: Arc<impl RibosomeT>,
    call_context: Arc<CallContext>,
    input: ChainQueryFilter,
) -> Result<Vec<Element>, WasmError> {
    tokio_helper::block_forever_on(async move {
        let elements: Vec<Element> = call_context
            .host_access
            .workspace()
            .write()
            .await
            .source_chain
            .query(&input)
            .map_err(|source_chain_error| WasmError::Host(source_chain_error.to_string()))?;
        Ok(elements)
    })
}

#[cfg(test)]
#[cfg(feature = "slow_tests")]
pub mod slow_tests {
    use crate::{core::ribosome::ZomeCallHostAccess, fixt::ZomeCallHostAccessFixturator};
    use ::fixt::prelude::*;
    use hdk::prelude::*;
    use holochain_lmdb::test_utils::TestEnvironment;
    use query::ChainQueryFilter;

    use holochain_wasm_test_utils::TestWasm;

    // TODO: use this setup function to DRY up a lot of duplicated code
    async fn setup() -> (TestEnvironment, ZomeCallHostAccess) {
        let test_env = holochain_lmdb::test_utils::test_cell_env();
        let env = test_env.env();

        let mut workspace =
            crate::core::workflow::CallZomeWorkspace::new(env.clone().into()).unwrap();

        // commits fail validation if we don't do genesis
        crate::core::workflow::fake_genesis(&mut workspace.source_chain)
            .await
            .unwrap();

        let workspace_lock = crate::core::workflow::CallZomeWorkspaceLock::new(workspace);
        let mut host_access = fixt!(ZomeCallHostAccess);
        host_access.workspace = workspace_lock;
        (test_env, host_access)
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn query_smoke_test() {
        let (_test_env, host_access) = setup().await;

        let _hash_a: EntryHash =
            crate::call_test_ribosome!(host_access, TestWasm::Query, "add_path", "a".to_string());
        let _hash_b: EntryHash =
            crate::call_test_ribosome!(host_access, TestWasm::Query, "add_path", "b".to_string());

        let elements: Vec<Element> = crate::call_test_ribosome!(
            host_access,
            TestWasm::Query,
            "query",
            ChainQueryFilter::default()
        );

        assert_eq!(elements.len(), 5);
    }
}
