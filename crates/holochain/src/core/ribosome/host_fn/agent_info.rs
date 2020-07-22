use crate::core::ribosome::error::RibosomeResult;
use crate::core::ribosome::wasm_ribosome::WasmRibosome;
use crate::core::ribosome::CallContext;
use crate::core::state::source_chain::SourceChainResult;
use crate::core::workflow::call_zome_workflow::InvokeZomeWorkspace;
use futures::FutureExt;
use holo_hash::AgentPubKey;
use holochain_zome_types::agent_info::AgentInfo;
use holochain_zome_types::AgentInfoInput;
use holochain_zome_types::AgentInfoOutput;
use must_future::MustBoxFuture;
use std::sync::Arc;

#[allow(clippy::extra_unused_lifetimes)]
pub fn agent_info<'a>(
    _ribosome: Arc<WasmRibosome>,
    call_context: Arc<CallContext>,
    _input: AgentInfoInput,
) -> RibosomeResult<AgentInfoOutput> {
    let call =
        |workspace: &'a InvokeZomeWorkspace| -> MustBoxFuture<'a, SourceChainResult<AgentPubKey>> {
            async move { Ok(workspace.source_chain.agent_pubkey().await?) }
                .boxed()
                .into()
        };
    let agent_pubkey: AgentPubKey =
        tokio_safe_block_on::tokio_safe_block_forever_on(async move {
            unsafe { call_context.host_access.workspace().apply_ref(call).await }
        })??;

    Ok(AgentInfoOutput::new(AgentInfo {
        agent_pubkey: agent_pubkey.clone(),
        // @todo these were in redux, what to do here?
        agent_initial_pubkey: agent_pubkey.clone(),
        agent_latest_pubkey: agent_pubkey,
    }))
}

#[cfg(test)]
#[cfg(feature = "slow_tests")]
pub mod test {
    use crate::core::state::workspace::Workspace;
    use crate::fixt::ZomeCallHostAccessFixturator;
    use fixt::prelude::*;
    use holo_hash_core::AgentPubKey;
    use holochain_state::env::ReadManager;
    use holochain_wasm_test_utils::TestWasm;
    use holochain_zome_types::AgentInfoInput;
    use holochain_zome_types::AgentInfoOutput;

    #[tokio::test(threaded_scheduler)]
    async fn invoke_import_agent_info_test() {
        let env = holochain_state::test_utils::test_cell_env();
        let dbs = env.dbs().await;
        let env_ref = env.guard().await;
        let reader = env_ref.reader().unwrap();
        let mut workspace = crate::core::workflow::InvokeZomeWorkspace::new(&reader, &dbs).unwrap();

        let (_g, raw_workspace) = crate::core::workflow::unsafe_invoke_zome_workspace::UnsafeInvokeZomeWorkspace::from_mut(&mut workspace);

        let mut host_access = fixt!(ZomeCallHostAccess);
        host_access.workspace = raw_workspace;

        let agent_info: AgentInfoOutput = crate::call_test_ribosome!(
            host_access,
            TestWasm::Imports,
            "agent_info",
            AgentInfoInput::new(())
        );
        assert_eq!(
            agent_info.inner_ref().agent_pubkey,
            AgentPubKey::from_raw_bytes(vec![0xdb; 36]),
        );
    }
}
