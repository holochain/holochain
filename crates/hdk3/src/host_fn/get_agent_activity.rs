use crate::prelude::*;

pub fn get_agent_activity(
    agent: AgentPubKey,
    query: ChainQueryFilter,
    request: ActivityRequest,
) -> HdkResult<AgentActivity> {
    Ok(host_call::<GetAgentActivityInput, GetAgentActivityOutput>(
        __get_agent_activity,
        &GetAgentActivityInput::new((agent, query, request)),
    )?
    .into_inner())
}
