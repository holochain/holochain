use crate::prelude::*;

/// Query the _headers_ of a remote agent's chain.
///
/// The agent activity is only the headers of their source chain.
/// The agent activity is held by the neighbourhood centered on the agent's public key, rather than a content hash like the rest of the DHT.
///
/// The agent activity can be filtered with [ `ChainQueryFilter` ] like a local chain query.
pub fn get_agent_activity(
    agent: AgentPubKey,
    query: ChainQueryFilter,
    request: ActivityRequest,
) -> ExternResult<AgentActivity> {
    HDK.get()
        .ok_or(WasmError::Guest(HDK_NOT_REGISTERED.to_string()))?
        .get_agent_activity(GetAgentActivityInput::new(agent, query, request))
}

/// Walks the source chain in reverse (latest to oldest) filtering by header and/or entry type
///
/// Given a header and entry type, returns an [ `Vec<Element>` ]
///
/// @todo document this better with examples
/// @todo do we want to return elements rather than hashes?
/// @todo implement cap grant/claim usage in terms of query
pub fn query(filter: ChainQueryFilter) -> ExternResult<Vec<Element>> {
    HDK.get()
        .ok_or(WasmError::Guest(HDK_NOT_REGISTERED.to_string()))?
        .query(filter)
}
