use crate::core::ribosome::CallContext;
use crate::core::ribosome::RibosomeT;
use holochain_types::prelude::*;
use holochain_wasmer_host::prelude::WasmError;
use std::sync::Arc;

#[allow(clippy::extra_unused_lifetimes)]
pub fn agent_info<'a>(
    _ribosome: Arc<impl RibosomeT>,
    call_context: Arc<CallContext>,
    _input: (),
) -> Result<AgentInfo, WasmError> {
    let agent_pubkey = call_context
        .host_access
        .workspace()
        .source_chain()
        .agent_pubkey()
        .clone();
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

    use holochain_state::host_fn_workspace::HostFnWorkspace;
    use holochain_types::prelude::*;
    use holochain_types::test_utils::fake_agent_pubkey_1;
    use holochain_wasm_test_utils::TestWasm;

    #[tokio::test(flavor = "multi_thread")]
    async fn invoke_import_agent_info_test() {
        let test_env = holochain_state::test_utils::test_cell_env();
        let test_cache = holochain_state::test_utils::test_cache_env();
        let env = test_env.env();
        let author = fake_agent_pubkey_1();
        crate::test_utils::fake_genesis(env.clone())
            .await
            .unwrap();
        let workspace = HostFnWorkspace::new(env.clone(), test_cache.env(), author).unwrap();

        let mut host_access = fixt!(ZomeCallHostAccess);
        host_access.workspace = workspace;

        let agent_info: AgentInfo =
            crate::call_test_ribosome!(host_access, TestWasm::AgentInfo, "agent_info", ());
        assert_eq!(agent_info.agent_initial_pubkey, fake_agent_pubkey_1(),);
        assert_eq!(agent_info.agent_latest_pubkey, fake_agent_pubkey_1(),);
    }
}
