use crate::core::ribosome::error::RibosomeResult;
use crate::core::ribosome::{CallContext, RibosomeT};
use holochain_zome_types::GetAgentActivityInput;
use holochain_zome_types::GetAgentActivityOutput;
use std::sync::Arc;

pub fn get_agent_activity(
    _ribosome: Arc<impl RibosomeT>,
    call_context: Arc<CallContext>,
    input: GetAgentActivityInput,
) -> RibosomeResult<GetAgentActivityOutput> {
    let (agent, query, request) = input.into_inner();
    let options = request.into();

    // Get the network from the context
    let network = call_context.host_access.network().clone();

    // timeouts must be handled by the network
    tokio_safe_block_on::tokio_safe_block_forever_on(async move {
        let activity = call_context
            .host_access
            .workspace()
            .write()
            .await
            .cascade(network)
            .get_agent_activity(agent, query, options)
            .await?;

        Ok(GetAgentActivityOutput::new(activity.into()))
    })
}

// we are relying on the create tests to show the commit/get round trip
// @see commit_entry.rs
