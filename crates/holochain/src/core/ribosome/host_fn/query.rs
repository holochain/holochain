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
            .source_chain()
            .query(input)
            .await
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
    use holochain_state::host_fn_workspace::HostFnWorkspace;
    use holochain_state::prelude::TestEnv;
    use query::ChainQueryFilter;

    use holochain_wasm_test_utils::TestWasm;

    // TODO: use this setup function to DRY up a lot of duplicated code
    async fn setup() -> (TestEnv, ZomeCallHostAccess) {
        let test_env = holochain_state::test_utils::test_cell_env();
        let test_cache = holochain_state::test_utils::test_cache_env();
        let env = test_env.env();
        let author = fake_agent_pubkey_1();
        crate::test_utils::fake_genesis(env.clone())
            .await
            .unwrap();
        let workspace = HostFnWorkspace::new(env.clone(), test_cache.env(), author).await.unwrap();
        let mut host_access = fixt!(ZomeCallHostAccess);
        host_access.workspace = workspace;
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
