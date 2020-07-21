use crate::core::ribosome::error::RibosomeResult;
use crate::core::ribosome::wasm_ribosome::WasmRibosome;
use crate::core::ribosome::CallContext;
use holochain_zome_types::globals::AgentInfo;
use holochain_zome_types::AgentInfoInput;
use holochain_zome_types::AgentInfoOutput;
use std::sync::Arc;

pub fn agent_info(
    _ribosome: Arc<WasmRibosome>,
    _host_context: Arc<CallContext>,
    _input: AgentInfoInput,
) -> RibosomeResult<AgentInfoOutput> {
    Ok(AgentInfoOutput::new(AgentInfo {
        agent_address: "todo".into(),  // @TODO
        agent_initial_hash: "".into(), // @TODO
        agent_latest_hash: "".into(),  // @TODO
    }))
}

#[cfg(test)]
#[cfg(feature = "slow_tests")]
pub mod test {
    use crate::core::state::workspace::Workspace;
    use crate::fixt::ZomeCallHostAccessFixturator;
    use fixt::prelude::*;
    use holochain_state::env::ReadManager;
    use holochain_wasm_test_utils::TestWasm;
    use holochain_zome_types::AgentInfoInput;
    use holochain_zome_types::{hash::HashString, AgentInfoOutput};

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
            agent_info.inner_ref().agent_address,
            HashString::from("todo")
        );
    }
}
