use crate::core::ribosome::error::RibosomeResult;
use crate::core::ribosome::wasm_ribosome::WasmRibosome;
use crate::core::ribosome::HostContext;
use crate::core::workflow::InvokeZomeWorkspace;
use futures::future::FutureExt;
use holo_hash::Hashed;
use holochain_state::error::DatabaseResult;
use holochain_types::test_utils::fake_agent_pubkey_1;
use holochain_zome_types::Entry;
use holochain_zome_types::GetEntryInput;
use holochain_zome_types::GetEntryOutput;
use must_future::MustBoxFuture;
use std::sync::Arc;

pub async fn get_entry<'a>(
    _ribosome: Arc<WasmRibosome>,
    host_context: Arc<HostContext>,
    _input: GetEntryInput,
) -> RibosomeResult<GetEntryOutput> {
    // TODO: EXAMPLE: This is only an example of how to use the workspace
    // and should be removed when this is implemented.
    let agent_pubkey = fake_agent_pubkey_1();
    let call =
        |workspace: &'a InvokeZomeWorkspace| -> MustBoxFuture<'a, DatabaseResult<Option<Entry>>> {
            async move {
                let cascade = workspace.cascade();
                let maybe_entry = cascade
                    .dht_get(&agent_pubkey.into())
                    .await?
                    .map(|e| e.into_inner().0);
                Ok(maybe_entry)
            }
            .boxed()
            .into()
        };
    let _entry = unsafe { host_context.workspace.apply_ref(call).await };
    todo!("Remove the above and implement get_entry")
}
