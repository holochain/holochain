use super::HostContext;
use super::WasmRibosome;
use crate::core::state::workspace::InvokeZomeWorkspace;
use futures::{future::BoxFuture, FutureExt};
use holochain_state::error::DatabaseResult;
use holochain_types::{entry::Entry, test_utils::fake_agent_pubkey_1};
use holochain_zome_types::GetEntryInput;
use holochain_zome_types::GetEntryOutput;
use std::sync::Arc;

pub async fn get_entry<'a>(
    _ribosome: Arc<WasmRibosome>,
    host_context: Arc<HostContext>,
    _input: GetEntryInput,
) -> GetEntryOutput {
    // TODO: EXAMPLE: This is only an example of how to use the workspace
    // and should be removed when this is implemented.
    let agent_pubkey = fake_agent_pubkey_1();
    let call =
        |workspace: &'a InvokeZomeWorkspace| -> BoxFuture<'a, DatabaseResult<Option<Entry>>> {
            async move {
                let cascade = workspace.cascade();
                cascade.dht_get(agent_pubkey.into()).await
            }
            .boxed()
        };
    let _entry = unsafe { host_context.workspace.apply_ref(call).await };
    todo!("Remove the above and implement get_entry")
}
