use crate::core::ribosome::CallContext;
use crate::core::ribosome::HostFnAccess;
use crate::core::ribosome::RibosomeT;
use holochain_types::prelude::*;
use holochain_wasmer_host::prelude::*;
use crate::core::ribosome::RibosomeError;
use std::sync::Arc;

#[allow(clippy::extra_unused_lifetimes)]
pub fn agent_info<'a>(
    _ribosome: Arc<impl RibosomeT>,
    call_context: Arc<CallContext>,
    _input: (),
) -> Result<AgentInfo, RuntimeError> {
    match HostFnAccess::from(&call_context.host_context()) {
        HostFnAccess {
            agent_info: Permission::Allow,
            ..
        } => {
            let agent_pubkey = call_context
                .host_context
                .workspace()
                .source_chain()
                .as_ref()
                .expect("Must have source chain if agent_info access is given")
                .agent_pubkey()
                .clone();
            Ok(AgentInfo {
                agent_initial_pubkey: agent_pubkey.clone(),
                agent_latest_pubkey: agent_pubkey,
                chain_head: call_context
                    .host_context
                    .workspace()
                    .source_chain()
                    .as_ref()
                    .expect("Must have source chain if agent_info access is given")
                    .chain_head()
                    .map_err(|e| wasm_error!(WasmErrorInner::Host(e.to_string())))?,
            })
        }
        _ => Err(wasm_error!(WasmErrorInner::Host(RibosomeError::HostFnPermissions(
            call_context.zome.zome_name().clone(),
            call_context.function_name().clone(),
            "agent_info".into()
        ).to_string())).into())
    }
}

#[cfg(test)]
#[cfg(feature = "slow_tests")]
pub mod test {
    use holochain_wasm_test_utils::TestWasm;
    use crate::core::ribosome::wasm_test::RibosomeTestFixture;
    use hdk::prelude::*;

    #[tokio::test(flavor = "multi_thread")]
    async fn agent_info_test() {
        observability::test_run().ok();
        let RibosomeTestFixture {
            conductor,
            alice,
            alice_pubkey,
            ..
        } = RibosomeTestFixture::new(TestWasm::AgentInfo).await;

        let call_info: CallInfo = conductor.call(&alice, "call_info", ()).await;
        let agent_info: AgentInfo = conductor.call(&alice, "agent_info", ()).await;
        assert_eq!(agent_info.agent_initial_pubkey, alice_pubkey);
        assert_eq!(agent_info.agent_latest_pubkey, alice_pubkey);

        assert_eq!(agent_info.chain_head.1, call_info.as_at.1 + 1,);

        let call_info_1: CallInfo = conductor.call(&alice, "call_info", ()).await;
        let agent_info_1: AgentInfo = conductor.call(&alice, "agent_info", ()).await;
        assert_eq!(agent_info_1.chain_head.1, call_info_1.as_at.1 + 1,);
    }
}
