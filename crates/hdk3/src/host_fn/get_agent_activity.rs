use crate::prelude::*;

pub fn get_agent_activity(
    agent: AgentPubKey,
    query: ChainQueryFilter,
    request: ActivityRequest,
) -> ExternResult<AgentActivity> {
    host_call::<GetAgentActivityInputInner, AgentActivity>(
        __get_agent_activity,
        GetAgentActivityInputInner::new(agent, query, request),
    )
}
