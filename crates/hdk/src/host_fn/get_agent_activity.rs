use crate::prelude::*;

pub fn get_agent_activity(
    agent: AgentPubKey,
    query: ChainQueryFilter,
    request: ActivityRequest,
) -> HdkResult<AgentActivity> {
    host_fn!(
        __get_agent_activity,
        GetAgentActivityInput::new((agent, query, request)),
        GetAgentActivityOutput
    )
}
