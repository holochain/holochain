use crate::core::ribosome::CallContext;
use crate::core::ribosome::RibosomeT;
use holochain_types::prelude::*;
use std::sync::Arc;
use holochain_wasmer_host::prelude::WasmError;

#[allow(clippy::extra_unused_lifetimes)]
pub fn agent_info<'a>(
    _ribosome: Arc<impl RibosomeT>,
    call_context: Arc<CallContext>,
    _input: (),
) -> Result<AgentInfo, WasmError> {
    let agent_pubkey = tokio_safe_block_on::tokio_safe_block_forever_on(async move {
        let lock = call_context.host_access.workspace().read().await;
        lock.source_chain.agent_pubkey()
    })
    .map_err(|source_chain_error| WasmError::Host(source_chain_error.to_string()))?;
    Ok(AgentInfo {
        agent_initial_pubkey: agent_pubkey.clone(),
        agent_latest_pubkey: agent_pubkey,
    })
}

#[cfg(test)]
#[cfg(feature = "slow_tests")]
pub mod test {
    use crate::fixt::ZomeCallHostAccessFixturator;
    use ::fixt::prelude::*;

    use holochain_types::prelude::*;
    use holochain_types::test_utils::fake_agent_pubkey_1;
    use holochain_wasm_test_utils::TestWasm;

    #[tokio::test(threaded_scheduler)]
    async fn invoke_import_agent_info_test() {
        let test_env = holochain_lmdb::test_utils::test_cell_env();
        let env = test_env.env();
        let mut workspace =
            crate::core::workflow::CallZomeWorkspace::new(env.clone().into()).unwrap();

        crate::core::workflow::fake_genesis(&mut workspace.source_chain)
            .await
            .unwrap();

        let workspace_lock = crate::core::workflow::CallZomeWorkspaceLock::new(workspace);

        let mut host_access = fixt!(ZomeCallHostAccess);
        host_access.workspace = workspace_lock;

        let agent_info: AgentInfo = crate::call_test_ribosome!(
            host_access,
            TestWasm::AgentInfo,
            "agent_info",
            ()
        );
        assert_eq!(
            agent_info.agent_initial_pubkey,
            fake_agent_pubkey_1(),
        );
        assert_eq!(
            agent_info.agent_latest_pubkey,
            fake_agent_pubkey_1(),
        );
    }
}
