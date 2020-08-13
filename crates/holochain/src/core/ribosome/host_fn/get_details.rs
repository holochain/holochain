use crate::core::ribosome::error::RibosomeResult;
use crate::core::ribosome::{CallContext, RibosomeT};
use crate::core::state::cascade::error::CascadeResult;
use crate::core::workflow::CallZomeWorkspace;
use futures::future::FutureExt;
use holochain_zome_types::{metadata::Details, GetDetailsInput, GetDetailsOutput};
use must_future::MustBoxFuture;
use std::sync::Arc;

#[allow(clippy::extra_unused_lifetimes)]
pub fn get_details<'a>(
    _ribosome: Arc<impl RibosomeT>,
    call_context: Arc<CallContext>,
    input: GetDetailsInput,
) -> RibosomeResult<GetDetailsOutput> {
    let (hash, options) = input.into_inner();

    // Get the network from the context
    let network = call_context.host_access.network().clone();

    let call =
        |workspace: &'a mut CallZomeWorkspace| -> MustBoxFuture<'a, CascadeResult<Option<Details>>> {
            async move {
                let mut cascade = workspace.cascade(network);
                Ok(cascade.get_details(hash, options.into()).await?)
            }
            .boxed()
            .into()
        };
    // timeouts must be handled by the network
    let maybe_details: Option<Details> =
        tokio_safe_block_on::tokio_safe_block_forever_on(async move {
            unsafe { call_context.host_access.workspace().apply_mut(call).await }
        })
        .map_err(Box::new)??;
    Ok(GetDetailsOutput::new(maybe_details))
}

// we are relying on the commit entry tests to show the commit/get round trip
// @see commit_entry.rs
