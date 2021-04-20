use self::get_agent_activity_query::hashes::GetAgentActivityQuery;
use self::get_entry_ops_query::GetEntryOpsQuery;
use self::get_links_ops_query::GetLinksOpsQuery;
use self::{
    get_agent_activity_query::deterministic::GetAgentActivityDeterministicQuery,
    get_element_query::GetElementOpsQuery,
};

use super::error::CascadeResult;
use holo_hash::AgentPubKey;
use holo_hash::HeaderHash;
use holochain_sqlite::db::ReadManager;
use holochain_state::query::Query;
use holochain_state::query::Txn;
use holochain_types::prelude::*;
use tracing::*;

pub use get_entry_ops_query::WireDhtOp;

#[cfg(test)]
mod test;

mod get_agent_activity_query;
mod get_element_query;
mod get_entry_ops_query;
mod get_links_ops_query;

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
pub fn handle_get_element(
    env: EnvRead,
    hash: HeaderHash,
    options: holochain_p2p::event::GetOptions,
) -> CascadeResult<WireElementOps> {
    let query = GetElementOpsQuery::new(hash, options);
    let results = env
        .conn()?
        .with_reader(|txn| query.run(Txn::from(txn.as_ref())))?;
    Ok(results)
}

#[instrument(skip(env))]
pub fn handle_get_agent_activity(
    env: EnvRead,
    agent: AgentPubKey,
    query: ChainQueryFilter,
    options: holochain_p2p::event::GetActivityOptions,
) -> CascadeResult<AgentActivityResponse<HeaderHash>> {
    let query = GetAgentActivityQuery::new(agent, query, options);
    let results = env
        .conn()?
        .with_reader(|txn| query.run(Txn::from(txn.as_ref())))?;
    Ok(results)
}

#[instrument(skip(env))]
pub fn handle_get_agent_activity_deterministic(
    env: EnvRead,
    agent: AgentPubKey,
    filter: AgentActivityFilterDeterministic,
    options: holochain_p2p::event::GetActivityOptions,
) -> CascadeResult<AgentActivityResponseDeterministic> {
    let query = GetAgentActivityDeterministicQuery::new(agent, filter, options);
    let results = env
        .conn()?
        .with_reader(|txn| query.run(Txn::from(txn.as_ref())))?;
    Ok(results)
}

#[instrument(skip(env, _options))]
pub fn handle_get_links(
    env: EnvRead,
    link_key: WireLinkKey,
    _options: holochain_p2p::event::GetLinksOptions,
) -> CascadeResult<WireLinkOps> {
    let query = GetLinksOpsQuery::new(link_key);
    let results = env
        .conn()?
        .with_reader(|txn| query.run(Txn::from(txn.as_ref())))?;
    Ok(results)
}
