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
    HDK.with(|h| {
        h.borrow()
            .get_agent_activity(GetAgentActivityInput::new(agent, query, request))
    })
}

/// Walks the source chain in reverse (latest to oldest) filtering by header and/or entry type
///
/// Given a header and entry type, returns an [ `Vec<Element>` ]
///
/// @todo document this better with examples after we make query do all the things we want.
/// @todo implement cap grant/claim usage in terms of query
/// @todo have ability to hash-bound query other agent's chains based on agent activity
/// @todo tie query into validation so we track dependencies e.g. validation packages
/// @todo decide which direction we want to iterate in (paramaterise query?)
/// @todo more expresivity generally?
pub fn query(filter: ChainQueryFilter) -> ExternResult<Vec<Element>> {
    HDK.with(|h| h.borrow().query(filter))
}
