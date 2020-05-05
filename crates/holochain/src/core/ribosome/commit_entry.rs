use super::HostContext;
use super::WasmRibosome;
use crate::core::state::{source_chain::SourceChainResult, workspace::InvokeZomeWorkspace};
use futures::{future::BoxFuture, FutureExt};
use holochain_types::{
    chain_header::ChainHeader, entry::Entry, header, test_utils::fake_agent_pubkey_1,
};
use holochain_zome_types::CommitEntryInput;
use holochain_zome_types::CommitEntryOutput;
use std::sync::Arc;

pub async fn commit_entry<'a>(
    _ribosome: Arc<WasmRibosome>,
    host_context: Arc<HostContext>,
    _input: CommitEntryInput,
) -> CommitEntryOutput {
    // Example of mutating source chain

    let call = |workspace: &'a mut InvokeZomeWorkspace| -> BoxFuture<'a, SourceChainResult<()>> {
        async move {
            let source_chain = &mut workspace.source_chain;
            let agent_pubkey = fake_agent_pubkey_1();
            let agent_entry = Entry::Agent(agent_pubkey.clone());
            let agent_header = ChainHeader::EntryCreate(header::EntryCreate {
                timestamp: chrono::Utc::now().timestamp().into(),
                author: agent_pubkey.clone(),
                prev_header: source_chain.chain_head().unwrap().clone(),
                entry_type: header::EntryType::AgentPubKey,
                entry_address: agent_pubkey.clone().into(),
            });
            source_chain.put(agent_header, Some(agent_entry)).await
        }
        .boxed()
    };
    let _result = unsafe { host_context.workspace.apply_mut(call).await };
    unimplemented!();
}
