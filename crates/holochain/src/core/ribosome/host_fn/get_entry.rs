use crate::core::ribosome::error::RibosomeResult;
use crate::core::ribosome::wasm_ribosome::WasmRibosome;
use crate::core::ribosome::HostContext;
use holochain_zome_types::GetEntryInput;
use holochain_zome_types::GetEntryOutput;
use std::sync::Arc;
use holochain_types::test_utils::fake_agent_pubkey_1;
use crate::core::workflow::InvokeZomeWorkspace;
use futures::future::BoxFuture;
use holochain_state::error::DatabaseResult;
use holochain_zome_types::Entry;
use futures::future::FutureExt;

pub async fn get_entry<'a>(
    _ribosome: Arc<WasmRibosome>,
    host_context: Arc<HostContext>,
    _input: GetEntryInput,
) -> RibosomeResult<GetEntryOutput> {
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
