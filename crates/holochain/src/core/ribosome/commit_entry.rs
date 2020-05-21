use super::HostContext;
use super::WasmRibosome;
use crate::core::state::source_chain::SourceChainResult;
use crate::core::workflow::InvokeZomeWorkspace;
use futures::{future::BoxFuture, FutureExt};
use holochain_types::{
    composite_hash::HeaderAddress, entry::Entry, header, header::Header, test_utils::fake_agent_pubkey_1,
    Timestamp,
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
    // TODO: EXAMPLE: This is only an example of how to use the workspace
    // and should be removed when this is implemented.
    let call = |workspace: &'a mut InvokeZomeWorkspace| -> BoxFuture<'a, SourceChainResult<HeaderAddress>> {
        async move {
            let source_chain = &mut workspace.source_chain;
            let agent_pubkey = fake_agent_pubkey_1();
            let agent_entry = Entry::Agent(agent_pubkey.clone());
            let agent_header = Header::EntryCreate(header::EntryCreate {
                author: agent_pubkey.clone(),
                timestamp: Timestamp::now(),
                header_seq: 0,
                prev_header: source_chain.chain_head().unwrap().clone(),
                entry_type: header::EntryType::AgentPubKey,
                entry_hash: agent_pubkey.clone().into(),
            });
            source_chain.put(agent_header, Some(agent_entry)).await
        }
        .boxed()
    };
    let _result = unsafe { host_context.workspace.apply_mut(call).await };
    todo!("Remove the above and implement commit_entry")
}
