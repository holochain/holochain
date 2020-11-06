use crate::prelude::*;
use holo_hash::AgentPubKey;
use holochain_wasmer_guest::SerializedBytesError;
use holochain_zome_types::query::{ActivityRequest, AgentActivity, ChainQueryFilter};

use crate::host_fn;

pub fn get_agent_activity(
    agent: AgentPubKey,
    query: ChainQueryFilter,
    request: ActivityRequest,
) -> Result<AgentActivity, SerializedBytesError> {
    host_fn!(
        __get_agent_activity,
        GetAgentActivityInput::new((agent, query, request)),
        GetAgentActivityOutput
    )
}
