use crate::nucleus::ribosome::error::RibosomeResult;
use crate::nucleus::ribosome::CallContext;
use crate::nucleus::ribosome::RibosomeT;
use holochain_zome_types::GetInput;
use holochain_zome_types::GetOutput;
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

    // timeouts must be handled by the network
    tokio_safe_block_on::tokio_safe_block_forever_on(async move {
        let maybe_element = call_context
            .host_access
            .workspace()
            .write()
            .await
            .cascade(network)
            .dht_get(hash, options.into())
            .await?;

        Ok(GetOutput::new(maybe_element))
    })
}

// we are relying on the create tests to show the commit/get round trip
// @see commit_entry.rs
