use crate::core::ribosome::error::RibosomeResult;
use crate::core::ribosome::{CallContext, RibosomeT};
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
    call_context: Arc<CallContext>,
    input: GetEntryInput,
) -> RibosomeResult<GetEntryOutput> {
    let (hash, _options) = input.into_inner();
    let cascade_hash = hash.try_into()?;
    let call =
        |workspace: &'a mut InvokeZomeWorkspace| -> MustBoxFuture<'a, DatabaseResult<Option<Entry>>> {
            async move {
                // TODO: Get the network from the context
                let network = todo!("Get the nework");
                let cascade = workspace.cascade(network);
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
            unsafe { call_context.host_access.workspace().apply_mut(call).await }
        })??;
    Ok(GetEntryOutput::new(maybe_entry))
}
