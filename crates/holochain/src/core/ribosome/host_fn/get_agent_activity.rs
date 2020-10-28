use crate::core::ribosome::error::RibosomeResult;
use crate::core::ribosome::{CallContext, RibosomeT};
use holochain_p2p::actor::GetActivityOptions;
use holochain_zome_types::GetAgentActivityOutput;
use holochain_zome_types::{query::ActivityRequest, GetAgentActivityInput};
use std::sync::Arc;

pub fn get_agent_activity(
    _ribosome: Arc<impl RibosomeT>,
    call_context: Arc<CallContext>,
    input: GetAgentActivityInput,
) -> RibosomeResult<GetAgentActivityOutput> {
    dbg!();
    let (agent, query, activity_request) = input.into_inner();
    dbg!();
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
    dbg!();

    // Get the network from the context
    let network = call_context.host_access.network().clone();

    dbg!();
    // timeouts must be handled by the network
    tokio_safe_block_on::tokio_safe_block_forever_on(async move {
    dbg!();
        let activity = call_context
            .host_access
            .workspace()
            .write()
            .await
            .cascade(network)
            .get_agent_activity(agent, query, options)
            .await?;

    dbg!();
        Ok(GetAgentActivityOutput::new(activity.into()))
    })
}

// we are relying on the create tests to show the commit/get round trip
// @see commit_entry.rs
