use super::HostContext;
use super::WasmRibosome;
use crate::core::state::source_chain::SourceChain;
use holochain_state::prelude::Reader;
use holochain_types::test_utils::fake_agent_hash;
use holochain_zome_types::GetEntryInput;
use holochain_zome_types::GetEntryOutput;
use std::sync::Arc;

pub async fn get_entry(
    _ribosome: Arc<WasmRibosome>,
    host_context: Arc<HostContext>,
    _input: GetEntryInput,
) -> GetEntryOutput {
    let call = |source_chain: &SourceChain<Reader>| {
        let agent_hash = fake_agent_hash("unsafe agent");
        source_chain.get_entry(agent_hash.into())
    };
    let _entry = unsafe { host_context.source_chain.apply_ref(call) };
    unimplemented!();
}
