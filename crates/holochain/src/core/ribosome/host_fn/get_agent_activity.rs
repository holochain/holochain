use crate::core::ribosome::CallContext;
use crate::core::ribosome::RibosomeT;
use holochain_cascade::Cascade;
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
    tokio_helper::runtime_block_on(async move {
        let workspace = call_context.host_access.workspace();
        let mut cascade = Cascade::from_workspace_network(workspace, network);
        let activity = cascade
            .get_agent_activity(agent_pubkey, chain_query_filter, options)
            .await
            .map_err(|cascade_error| WasmError::Host(cascade_error.to_string()))?;

        Ok(activity.into())
    })
}

// we are relying on the create tests to show the commit/get round trip
// See commit_entry.rs
