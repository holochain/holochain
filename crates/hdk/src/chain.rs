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

/// Query for source chain records for the current agent, with optional filtering.
///
/// Applies filters to the source chain, and returns a list of matching [`Record`]s.
///
/// The primary filter is by range, through a combination of sequence numbers or action hashes.
/// Each has quite different characteristics, so the choice must be made carefully.
///
/// ## Unbounded query
///
/// Using [`ChainQueryFilterRange::Unbounded`] does not apply any bounds when retrieving data
/// from the database.
///
/// Its characteristics are equivalent to [`ChainQueryFilterRange::ActionSeqRange`] with `start=0`
/// and `end=u32::MAX`.
///
/// ## Sequence Number Ranges
///
/// Using [`ChainQueryFilterRange::ActionSeqRange`] fetches all records whose action sequence
/// numbers are between the specified start and end bounds (inclusive) from the database.
///
/// In the case of chain forks, this may return multiple records for the same sequence number.
/// Since the filter does not give the query a way to pick a fork, all matching records are
/// returned.
///
/// The filters [`ChainQueryFilter::action_type`] and [`ChainQueryFilter::entry_type`] are
/// applied as part of the database query, making them reasonably efficient to use.
///
/// ## Hash bounded queries
///
/// Using either [`ChainQueryFilterRange::ActionHashTerminated`] or [`ChainQueryFilterRange::ActionHashRange`]
/// will choose action sequence numbers based on the action hashes provided. It will then return
/// all records whose action sequence numbers are between the calculated start and end bounds
/// (inclusive).
///
/// In either case, the presence of an action hash that defines the latest entry in a chain, allows
/// choosing a specific chain. This means that even if forks are present, the query will only
/// return records from the chain defined by the latest action hash.
///
/// For chain forks to be handled correctly, it is not possible to apply other filters during the
/// database query. All relevant records must be loaded, and a chain reconstruction step must be
/// performed before any other filters are applied. This means that using hash-bounded queries may
/// be significantly less efficient than other query types.
//
// @todo document this better with examples after we make query do all the things we want.
// @todo implement cap grant/claim usage in terms of query
// @todo have ability to hash-bound query other agent's chains based on agent activity
// @todo decide which direction we want to iterate in (parameterize query?)
// @todo more expressive generally?
pub fn query(filter: ChainQueryFilter) -> ExternResult<Vec<Record>> {
    HDK.with(|h| h.borrow().query(filter))
}
