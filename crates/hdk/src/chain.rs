use crate::prelude::*;
pub use hdi::chain::*;

/// Query the current state of an agent's chain, including:
///
/// * The highest observed chain item (or items, in the case of a chain fork),
/// * A summary of whether the chain contains only valid items, contains at
///   least one invalid item, is forked, or is empty,
/// * Any warrants collected for invalid actions committed by the agent, and
/// * Action sequences and hashes of valid and rejected actions if desired (see
///   [`ActivityRequest::Full`]).
///
/// The agent activity is held by the neighborhood of the agent's public key.
///
/// If retrieving chain items along with the current state using
/// [`ActivityRequest::Full`], the chain items in
/// [`AgentActivity::valid_activity`] and [`AgentActivity::rejected_activity`]
/// can be filtered with [`ChainQueryFilter`] like a local chain query. This
/// filtering happens at the source before it sends the data to the receiver.
///
/// Parameters:
///
/// * `agent`: The agent to retrieve the status of.
/// * `query`: An optional filter for the resulting [`AgentActivity::valid_activity`]
///   and [`AgentActivity::rejected_activity`] values. This is only used when
///   the `request` argument is [`ActivityRequest::Full`].
/// * `request`: The type of data to retrieve -- a summary of current state, or
///   the summary plus hashes of chain actions matching the filter.
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

/// Walks the source chain in ascending order (oldest to latest) filtering by action and/or entry type
///
/// Given an action and entry type, returns an [`Vec<Record>`]
///
// @todo document this better with examples after we make query do all the things we want.
// @todo implement cap grant/claim usage in terms of query
// @todo have ability to hash-bound query other agent's chains based on agent activity
// @todo tie query into validation so we track dependencies e.g. validation packages
// @todo decide which direction we want to iterate in (paramaterise query?)
// @todo more expresivity generally?
pub fn query(filter: ChainQueryFilter) -> ExternResult<Vec<Record>> {
    HDK.with(|h| h.borrow().query(filter))
}
