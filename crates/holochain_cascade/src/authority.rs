use self::get_agent_activity_query::hashes::GetAgentActivityQuery;
use self::get_entry_ops_query::GetEntryOpsQuery;
use self::get_links_ops_query::GetLinksOpsQuery;
use self::{
    get_agent_activity_query::deterministic::DeterministicGetAgentActivityQuery,
    get_element_query::GetElementOpsQuery,
};

use super::error::CascadeResult;
use holo_hash::AgentPubKey;
use holo_hash::HeaderHash;
use holochain_state::query::Query;
use holochain_state::query::Txn;
use holochain_types::prelude::*;
use holochain_zome_types::agent_activity::DeterministicGetAgentActivityFilter;
use tracing::*;

#[cfg(test)]
mod test;

pub(crate) mod get_agent_activity_query;
pub(crate) mod get_element_query;
pub(crate) mod get_entry_ops_query;
pub(crate) mod get_links_ops_query;

#[instrument(skip(state_env))]
pub async fn handle_get_entry(
    state_env: DbRead<DbKindDht>,
    hash: EntryHash,
    _options: holochain_p2p::event::GetOptions,
) -> CascadeResult<WireEntryOps> {
    let query = GetEntryOpsQuery::new(hash);
    let results = state_env
        .async_reader(move |txn| query.run(Txn::from(&txn)))
        .await?;
    Ok(results)
}

#[tracing::instrument(skip(env))]
pub async fn handle_get_element(
    env: DbRead<DbKindDht>,
    hash: HeaderHash,
    options: holochain_p2p::event::GetOptions,
) -> CascadeResult<WireElementOps> {
    let query = GetElementOpsQuery::new(hash, options);
    let results = env
        .async_reader(move |txn| query.run(Txn::from(&txn)))
        .await?;
    Ok(results)
}

#[instrument(skip(env))]
pub async fn handle_get_agent_activity(
    env: DbRead<DbKindDht>,
    agent: AgentPubKey,
    query: ChainQueryFilter,
    options: holochain_p2p::event::GetActivityOptions,
) -> CascadeResult<AgentActivityResponse<HeaderHash>> {
    let query = GetAgentActivityQuery::new(agent, query, options);
    let results = env
        .async_reader(move |txn| query.run(Txn::from(&txn)))
        .await?;
    Ok(results)
}

#[instrument(skip(env))]
pub async fn handle_get_agent_activity_deterministic(
    env: DbRead<DbKindDht>,
    agent: AgentPubKey,
    filter: DeterministicGetAgentActivityFilter,
    options: holochain_p2p::event::GetActivityOptions,
) -> CascadeResult<DeterministicGetAgentActivityResponse> {
    let query = DeterministicGetAgentActivityQuery::new(agent, filter, options);
    let results = env
        .async_reader(move |txn| query.run(Txn::from(&txn)))
        .await?;
    Ok(results)
}

#[instrument(skip(env, _options))]
pub async fn handle_get_links(
    env: DbRead<DbKindDht>,
    link_key: WireLinkKey,
    _options: holochain_p2p::event::GetLinksOptions,
) -> CascadeResult<WireLinkOps> {
    let query = GetLinksOpsQuery::new(link_key);
    let results = env
        .async_reader(move |txn| query.run(Txn::from(&txn)))
        .await?;
    Ok(results)
}
