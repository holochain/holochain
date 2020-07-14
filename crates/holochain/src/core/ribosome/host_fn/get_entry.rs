use crate::core::ribosome::error::RibosomeResult;
use crate::core::ribosome::{HostContext, RibosomeT};
use crate::core::workflow::InvokeZomeWorkspace;
use futures::future::FutureExt;
use holo_hash::Hashed;
use holochain_state::error::DatabaseResult;
use holochain_zome_types::Entry;
use holochain_zome_types::GetEntryInput;
use holochain_zome_types::GetEntryOutput;
use must_future::MustBoxFuture;
use std::convert::TryInto;
use std::sync::Arc;

#[allow(clippy::extra_unused_lifetimes)]
pub fn get_entry<'a>(
    _ribosome: Arc<impl RibosomeT>,
    host_context: Arc<HostContext>,
    input: GetEntryInput,
) -> RibosomeResult<GetEntryOutput> {
    let (hash, _options) = input.into_inner();
    let cascade_hash = hash.try_into()?;
    let call =
        |workspace: &'a InvokeZomeWorkspace| -> MustBoxFuture<'a, DatabaseResult<Option<Entry>>> {
            async move {
                let cascade = workspace.cascade();
                // safe block on
                let maybe_entry = cascade
                    .dht_get(&cascade_hash)
                    .await?
                    .map(|e| e.into_content());
                Ok(maybe_entry)
            }
            .boxed()
            .into()
        };
    let maybe_entry: Option<Entry> =
        tokio_safe_block_on::tokio_safe_block_forever_on(async move {
            unsafe { host_context.workspace.apply_ref(call).await }
        })??;
    Ok(GetEntryOutput::new(maybe_entry))
}
