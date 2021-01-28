use crate::prelude::*;

pub fn get_agent_activity(
    agent: AgentPubKey,
    query: ChainQueryFilter,
    request: ActivityRequest,
) -> ExternResult<AgentActivity> {
    host_call::<GetAgentActivityInput, AgentActivity>(
        __get_agent_activity,
        GetAgentActivityInput::new(agent, query, request),
    )
}
