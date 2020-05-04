use super::HostContext;
use super::WasmRibosome;
use crate::core::state::source_chain::SourceChain;
use holochain_state::prelude::Reader;
use holochain_types::{
    chain_header::ChainHeader, entry::Entry, header, test_utils::fake_agent_pubkey_1,
};
use holochain_zome_types::CommitEntryInput;
use holochain_zome_types::CommitEntryOutput;
use std::sync::Arc;
use futures::FutureExt;

pub async fn commit_entry(
    _ribosome: Arc<WasmRibosome>,
    host_context: Arc<HostContext>,
    _input: CommitEntryInput,
) -> CommitEntryOutput {
    // Example of mutating source chain
    let call = |source_chain: &mut SourceChain<Reader>| {
        /* FIXME: Same lifetime issue as get_entry
        async move {
            let agent_pubkey = fake_agent_pubkey_1();
            let agent_entry = Entry::Agent(agent_pubkey.clone());
            let agent_header = ChainHeader::EntryCreate(header::EntryCreate {
                timestamp: chrono::Utc::now().timestamp().into(),
                author: agent_pubkey.clone(),
                prev_header: *source_chain.chain_head().unwrap(),
                entry_type: header::EntryType::AgentPubKey,
                entry_address: agent_pubkey.clone().into(),
            });
            source_chain.put(agent_header, Some(agent_entry)).await
        }
        .boxed()
        */
    };
    let _result = unsafe { host_context.source_chain.apply_mut(call) };
    unimplemented!();
}
