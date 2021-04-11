use self::get_element_query::GetElementOpsQuery;
use self::get_entry_ops_query::GetEntryOpsQuery;

use super::error::CascadeResult;
use holo_hash::AgentPubKey;
use holo_hash::HeaderHash;
use holochain_sqlite::db::ReadManager;
use holochain_state::query::Query;
use holochain_state::query::Txn;
use holochain_types::prelude::*;
use tracing::*;

pub use get_element_query::WireElementOps;
pub use get_entry_ops_query::WireDhtOp;
pub use get_entry_ops_query::WireEntryOps;

#[cfg(test)]
mod test;

mod get_element_query;
mod get_entry_ops_query;

// TODO: Move this to holochain types.
#[derive(Debug, PartialEq, Eq, Clone)]
pub enum WireOps {
    Entry(WireEntryOps),
    Element(WireElementOps),
}

#[instrument(skip(state_env))]
pub fn handle_get_entry(
    state_env: EnvRead,
    hash: EntryHash,
    _options: holochain_p2p::event::GetOptions,
) -> CascadeResult<WireEntryOps> {
    let query = GetEntryOpsQuery::new(hash);
    let results = state_env
        .conn()?
        .with_reader(|txn| query.run(Txn::from(txn.as_ref())))?;
    Ok(results)
}

#[tracing::instrument(skip(env))]
pub fn handle_get_element(env: EnvRead, hash: HeaderHash) -> CascadeResult<WireElementOps> {
    let query = GetElementOpsQuery::new(hash);
    let results = env
        .conn()?
        .with_reader(|txn| query.run(Txn::from(txn.as_ref())))?;
    Ok(results)
}

#[instrument(skip(env))]
pub fn handle_get_agent_activity(
    env: EnvRead,
    agent: AgentPubKey,
    filter: ChainQueryFilter,
    options: holochain_p2p::event::GetActivityOptions,
) -> CascadeResult<AgentActivityResponse> {
    todo!()
    // let query = GetAgentActivityQuery::new(agent, filter, options);
    // let results = state_env
    //     .conn()?
    //     .with_reader(|txn| query.run(Txn::from(txn.as_ref())))?;
    // Ok(results)
}

#[instrument(skip(_env, _options))]
pub fn handle_get_links(
    _env: EnvRead,
    _link_key: WireLinkMetaKey,
    _options: holochain_p2p::event::GetLinksOptions,
) -> CascadeResult<GetLinksResponse> {
    todo!()
}
