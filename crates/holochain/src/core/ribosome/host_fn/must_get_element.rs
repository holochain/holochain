use crate::core::ribosome::CallContext;
use crate::core::ribosome::RibosomeT;
use holochain_cascade::Cascade;
use holochain_types::prelude::*;
use holochain_wasmer_host::prelude::WasmError;
use std::sync::Arc;
use holochain_p2p::event::GetRequest;
use holochain_p2p::actor::GetOptions as NetworkGetOptions;

#[allow(clippy::extra_unused_lifetimes)]
pub fn must_get_element<'a>(
    _ribosome: Arc<impl RibosomeT>,
    call_context: Arc<CallContext>,
    input: MustGetElementInput,
) -> Result<Element, WasmError> {
    // Get the network from the context
    let network = call_context.host_access.network().clone();

    // timeouts must be handled by the network
    tokio_helper::block_forever_on(async move {
        let workspace = call_context.host_access.workspace();
        let mut cascade = Cascade::from_workspace_network(workspace, network);
        match cascade
            .retrieve_header(input.into_inner(),
            // Set every GetOptions manually here.
            // Using defaults is dangerous as it can undermine determinism.
            // We want refactors to explicitly consider this.
            NetworkGetOptions {
                remote_agent_count: None,
                timeout_ms: None,
                as_race: true,
                race_timeout_ms: None,
                // Never redirect as the returned entry must always match the hash.
                follow_redirects: false,
                // Ignore deletes.
                all_live_headers_with_metadata: true,
                // Redundant with retrieve_entry internals.
                request_type: GetRequest::Pending,
            })
            .await
            .map_err(|cascade_error| WasmError::Host(cascade_error.to_string()))? {
                Some(element) => Ok(element),
                None => Err()
            }

        Ok(maybe_element)
    })
}

// we are relying on the create tests to show the commit/get round trip
// See commit_entry.rs