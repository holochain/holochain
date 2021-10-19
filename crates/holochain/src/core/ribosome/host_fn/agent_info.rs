use crate::core::ribosome::CallContext;
use crate::core::ribosome::HostFnAccess;
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
            })
        }
        _ => unreachable!(),
    }
}

#[cfg(test)]
#[cfg(feature = "slow_tests")]
pub mod test {
    use crate::fixt::ZomeCallHostAccessFixturator;
    use ::fixt::prelude::*;

    use holochain_types::prelude::*;
    use holochain_types::test_utils::fake_agent_pubkey_1;
    use holochain_wasm_test_utils::TestWasm;

    #[tokio::test(flavor = "multi_thread")]
    async fn invoke_import_agent_info_test() {
        let host_access = fixt!(ZomeCallHostAccess, Predictable);

        let agent_info: AgentInfo =
            crate::call_test_ribosome!(host_access, TestWasm::AgentInfo, "agent_info", ()).unwrap();
        assert_eq!(agent_info.agent_initial_pubkey, fake_agent_pubkey_1(),);
        assert_eq!(agent_info.agent_latest_pubkey, fake_agent_pubkey_1(),);
    }
}
