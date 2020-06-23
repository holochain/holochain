use crate::core::ribosome::error::RibosomeResult;
use crate::core::ribosome::wasm_ribosome::WasmRibosome;
use crate::core::ribosome::HostContext;
use crate::core::workflow::InvokeZomeWorkspace;
use futures::future::FutureExt;
use holo_hash::Hashed;
use holochain_state::error::DatabaseResult;
use holochain_zome_types::Entry;
use holochain_zome_types::GetEntryInput;
use holochain_zome_types::GetEntryOutput;
use must_future::MustBoxFuture;
use std::sync::Arc;

pub async fn get_entry<'a>(
    _ribosome: Arc<WasmRibosome>,
    host_context: Arc<HostContext>,
    input: GetEntryInput,
) -> RibosomeResult<GetEntryOutput> {
    let (hash, _options) = input.into_inner();
    let call =
        |workspace: &'a InvokeZomeWorkspace| -> MustBoxFuture<'a, DatabaseResult<Option<Entry>>> {
            async move {
                let cascade = workspace.cascade();
                let maybe_entry = cascade
                    .dht_get(&hash.into())
                    .await?
                    .map(|e| e.into_content());
                Ok(maybe_entry)
            }
            .boxed()
            .into()
        };
    let maybe_entry: Option<Entry> = unsafe { host_context.workspace.apply_ref(call).await?? };
    Ok(GetEntryOutput::new(maybe_entry))
}
