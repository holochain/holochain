use super::HostContext;
use super::WasmRibosome;
use holochain_zome_types::CommitEntryInput;
use holochain_zome_types::CommitEntryOutput;
use std::sync::Arc;
use crate::core::state::source_chain::SourceChain;
use holochain_types::{entry::Entry, test_utils::fake_agent_hash};
use holochain_state::prelude::Reader;

pub async fn commit_entry(
    _ribosome: Arc<WasmRibosome>,
    host_context: Arc<HostContext>,
    _input: CommitEntryInput,
) -> CommitEntryOutput {
    // Example of mutating source chain 
    let call = |source_chain: &mut SourceChain<Reader>| {
        let agent_hash = fake_agent_hash("unsafe agent");
        let agent_entry = Entry::AgentKey(agent_hash.clone());
        source_chain.put_entry(agent_entry, &agent_hash)
    };
    let _result = unsafe { host_context.source_chain.apply_mut(call) };
    unimplemented!();
}
