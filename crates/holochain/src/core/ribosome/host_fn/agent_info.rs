use crate::core::ribosome::error::RibosomeResult;
use crate::core::ribosome::CallContext;
use crate::core::ribosome::RibosomeT;
use crate::core::state::source_chain::SourceChainResult;
use crate::core::workflow::call_zome_workflow::CallZomeWorkspace;
use futures::FutureExt;
use holo_hash::AgentPubKey;
use holochain_zome_types::agent_info::AgentInfo;
use holochain_zome_types::AgentInfoInput;
use holochain_zome_types::AgentInfoOutput;
use must_future::MustBoxFuture;
use std::sync::Arc;

#[allow(clippy::extra_unused_lifetimes)]
pub fn agent_info<'a>(
    _ribosome: Arc<impl RibosomeT>,
    call_context: Arc<CallContext>,
    _input: AgentInfoInput,
) -> RibosomeResult<AgentInfoOutput> {
    let call =
        |workspace: &'a CallZomeWorkspace| -> MustBoxFuture<'a, SourceChainResult<AgentPubKey>> {
            async move { Ok(workspace.source_chain.agent_pubkey().await?) }
                .boxed()
                .into()
        };
    let agent_pubkey: AgentPubKey =
        tokio_safe_block_on::tokio_safe_block_forever_on(async move {
            unsafe { call_context.host_access.workspace().apply_ref(call).await }
        })??;
    Ok(AgentInfoOutput::new(AgentInfo {
        agent_initial_pubkey: agent_pubkey.clone(),
        agent_latest_pubkey: agent_pubkey,
    }))
}

#[cfg(test)]
#[cfg(feature = "slow_tests")]
pub mod test {

    use crate::fixt::ZomeCallHostAccessFixturator;
    use fixt::prelude::*;

    use holochain_types::test_utils::fake_agent_pubkey_1;
    use holochain_wasm_test_utils::TestWasm;
    use holochain_zome_types::AgentInfoInput;
    use holochain_zome_types::AgentInfoOutput;

    #[tokio::test(threaded_scheduler)]
    async fn invoke_import_agent_info_test() {
        let env = holochain_state::test_utils::test_cell_env();
        let dbs = env.dbs().await;
        let mut workspace = crate::core::workflow::CallZomeWorkspace::new(env.clone().into(), &dbs)
            .await
            .unwrap();

        crate::core::workflow::fake_genesis(&mut workspace.source_chain)
            .await
            .unwrap();

        let (_g, raw_workspace) =
            crate::core::workflow::unsafe_call_zome_workspace::UnsafeCallZomeWorkspace::from_mut(
                &mut workspace,
            );

        let mut host_access = fixt!(ZomeCallHostAccess);
        host_access.workspace = raw_workspace;

        let agent_info: AgentInfoOutput = crate::call_test_ribosome!(
            host_access,
            TestWasm::AgentInfo,
            "agent_info",
            AgentInfoInput::new(())
        );
        assert_eq!(
            agent_info.inner_ref().agent_initial_pubkey,
            fake_agent_pubkey_1(),
        );
        assert_eq!(
            agent_info.inner_ref().agent_latest_pubkey,
            fake_agent_pubkey_1(),
        );
    }
}
