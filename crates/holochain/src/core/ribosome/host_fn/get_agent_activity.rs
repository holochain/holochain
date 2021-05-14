use crate::core::ribosome::CallContext;
use crate::core::ribosome::RibosomeT;
use holochain_p2p::actor::GetActivityOptions;
use holochain_types::prelude::*;
use holochain_wasmer_host::prelude::WasmError;
use std::sync::Arc;

pub fn get_agent_activity(
    _ribosome: Arc<impl RibosomeT>,
    call_context: Arc<CallContext>,
    input: GetAgentActivityInput,
) -> Result<AgentActivity, WasmError> {
    let GetAgentActivityInput {
        agent_pubkey,
        chain_query_filter,
        activity_request,
    } = input;
    let options = match activity_request {
        ActivityRequest::Status => GetActivityOptions {
            include_valid_activity: false,
            include_rejected_activity: false,
            ..Default::default()
        },
        ActivityRequest::Full => GetActivityOptions {
            include_valid_activity: true,
            include_rejected_activity: true,
            ..Default::default()
        },
    };

    // Get the network from the context
    let network = call_context.host_access.network().clone();

    // timeouts must be handled by the network
    tokio_helper::block_forever_on(async move {
        let activity = call_context
            .host_access
            .workspace()
            .write()
            .await
            .cascade(network)
            .get_agent_activity(agent_pubkey, chain_query_filter, options)
            .await
            .map_err(|cascade_error| WasmError::Host(cascade_error.to_string()))?;

        Ok(activity.into())
    })
}

// we are relying on the create tests to show the commit/get round trip
// See commit_entry.rs
