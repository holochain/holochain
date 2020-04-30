use super::HostContext;
use super::WasmRibosome;
use crate::core::state::source_chain::SourceChain;
use holochain_state::prelude::Reader;
use holochain_zome_types::CommitEntryInput;
use holochain_zome_types::CommitEntryOutput;
use std::sync::Arc;
use holochain_types::{entry::Entry, test_utils::fake_agent_hash};

pub async fn commit_entry(
    _ribosome: Arc<WasmRibosome>,
    host_context: Arc<HostContext>,
    _input: CommitEntryInput,
) -> CommitEntryOutput {
    let source_chain = host_context.source_chain as *mut SourceChain<Reader>;
    if let Some(source_chain) = unsafe { source_chain.as_mut() } {
        let agent_hash = fake_agent_hash("unsafe agent");
        let entry = Entry::AgentKey(agent_hash.clone());
        source_chain.put_entry(entry, &agent_hash);
    }
    unimplemented!();
}
