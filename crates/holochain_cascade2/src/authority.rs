use super::error::CascadeResult;
use holo_hash::AgentPubKey;
use holo_hash::HeaderHash;
// use holochain_sqlite::db::ReadManager;
// use holochain_state::query::entry::GetQuery;
// use holochain_state::query::Query;
use holochain_types::prelude::*;
use tracing::*;

#[cfg(test)]
mod test;

// TODO: Move this to holochain types.
// TODO: Don't duplicate the entry by sending full ops.
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct WireEntryOps {
    creates: Vec<DhtOp>,
    deletes: Vec<DhtOp>,
    updates: Vec<DhtOp>,
}

#[instrument(skip(_state_env))]
pub fn handle_get_entry(
    _state_env: EnvRead,
    hash: EntryHash,
    _options: holochain_p2p::event::GetOptions,
) -> CascadeResult<WireEntryOps> {
    let mut _query = todo!("Need a query that returns ops not SignedHeaderHashed");
    // let results = state_env
    //     .conn()?
    //     .with_reader(|txn| query.run(&[&txn], None))?;
}

#[tracing::instrument(skip(_env))]
pub fn handle_get_element(_env: EnvRead, _hash: HeaderHash) -> CascadeResult<GetElementResponse> {
    todo!()
}

#[instrument(skip(_env))]
pub fn handle_get_agent_activity(
    _env: EnvRead,
    _agent: AgentPubKey,
    _query: ChainQueryFilter,
    _options: holochain_p2p::event::GetActivityOptions,
) -> CascadeResult<AgentActivityResponse> {
    todo!()
}

#[instrument(skip(_env, _options))]
pub fn handle_get_links(
    _env: EnvRead,
    _link_key: WireLinkMetaKey,
    _options: holochain_p2p::event::GetLinksOptions,
) -> CascadeResult<GetLinksResponse> {
    todo!()
}
