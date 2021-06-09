use crate::core::ribosome::CallContext;
use crate::core::ribosome::RibosomeT;
use holochain_cascade::Cascade;
use holochain_types::prelude::*;
use holochain_wasmer_host::prelude::WasmError;
use std::sync::Arc;

#[allow(clippy::extra_unused_lifetimes)]
pub fn must_get_entry<'a>(
    _ribosome: Arc<impl RibosomeT>,
    call_context: Arc<CallContext>,
    input: GetInput,
) -> Result<Entry, WasmError> {
    let MustGetEntryInput {
        entry_hash
    } = input;

    // Get the network from the context
    let network = call_context.host_access.network().clone();

    // timeouts must be handled by the network
    tokio_helper::block_forever_on(async move {
        let workspace = call_context.host_access.workspace();
        let mut cascade = Cascade::from_workspace_network(workspace, network);
        let maybe_element = cascade
            .retrieve_entry(any_dht_hash, get_options)
            .await
            .map_err(|cascade_error| WasmError::Host(cascade_error.to_string()))?;

        Ok(maybe_element)
    })
}

// we are relying on the create tests to show the commit/get round trip
// See commit_entry.rs
