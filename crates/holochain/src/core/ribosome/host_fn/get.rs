use crate::core::ribosome::error::RibosomeResult;
use crate::core::ribosome::{CallContext, RibosomeT};
use crate::core::state::cascade::error::CascadeResult;
use crate::core::workflow::CallZomeWorkspace;
use futures::future::FutureExt;
use holochain_zome_types::element::Element;
use holochain_zome_types::GetInput;
use holochain_zome_types::GetOutput;
use must_future::MustBoxFuture;
use std::sync::Arc;

#[allow(clippy::extra_unused_lifetimes)]
pub fn get<'a>(
    _ribosome: Arc<impl RibosomeT>,
    call_context: Arc<CallContext>,
    input: GetInput,
) -> RibosomeResult<GetOutput> {
    let (hash, options) = input.into_inner();

    // Get the network from the context
    let network = call_context.host_access.network().clone();

    let call =
        |workspace: &'a mut CallZomeWorkspace| -> MustBoxFuture<'a, CascadeResult<Option<Element>>> {
            async move {
                let mut cascade = workspace.cascade(network);
                // safe block on
                let maybe_element = cascade
                    .dht_get(hash.clone(), options.into())
                    .await?;
                Ok(maybe_element)
            }
            .boxed()
            .into()
        };
    // timeouts must be handled by the network
    let maybe_entry: Option<Element> =
        tokio_safe_block_on::tokio_safe_block_forever_on(async move {
            unsafe { call_context.host_access.workspace().apply_mut(call).await }
        })??;
    Ok(GetOutput::new(maybe_entry))
}

// we are relying on the commit entry tests to show the commit/get round trip
// @see commit_entry.rs
