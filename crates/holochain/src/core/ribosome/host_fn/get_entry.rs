use crate::core::ribosome::error::RibosomeResult;
use crate::core::ribosome::{CallContext, RibosomeT};
use crate::core::workflow::CallZomeWorkspace;
use futures::future::FutureExt;
use holochain_state::error::DatabaseResult;
use holochain_zome_types::Entry;
use holochain_zome_types::GetEntryInput;
use holochain_zome_types::GetEntryOutput;
use must_future::MustBoxFuture;
use std::sync::Arc;

#[allow(clippy::extra_unused_lifetimes)]
pub fn get_entry<'a>(
    _ribosome: Arc<impl RibosomeT>,
    call_context: Arc<CallContext>,
    input: GetEntryInput,
) -> RibosomeResult<GetEntryOutput> {
    let (hash, _options) = input.into_inner();

    // Get the network from the context
    let network = call_context.host_access.network().clone();

    let call =
        |workspace: &'a mut CallZomeWorkspace| -> MustBoxFuture<'a, DatabaseResult<Option<Entry>>> {
            async move {
                let cascade = workspace.cascade(network);
                Ok(cascade.dht_get(&hash).await?.map(|e| e.into_content()))
            }
            .boxed()
            .into()
        };
    // timeouts must be handled by the network
    let maybe_entry: Option<Entry> =
        tokio_safe_block_on::tokio_safe_block_forever_on(async move {
            unsafe { call_context.host_access.workspace().apply_mut(call).await }
        })??;
    Ok(GetEntryOutput::new(maybe_entry))
}

// we are relying on the commit entry tests to show the commit/get round trip
// @see commit_entry.rs
