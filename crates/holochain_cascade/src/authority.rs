//! Functions for the various authorities to handle queries

use self::get_agent_activity_query::actions::GetAgentActivityActionsQuery;
use self::get_agent_activity_query::must_get_agent_activity::must_get_agent_activity;
use self::get_entry_ops_query::GetEntryOpsQuery;
use self::get_links_ops_query::GetLinksOpsQuery;
use self::{
    get_agent_activity_query::deterministic::DeterministicGetAgentActivityQuery,
    get_record_query::GetRecordOpsQuery,
};

use super::error::CascadeResult;
use crate::authority::get_agent_activity_query::hashes::GetAgentActivityHashesQuery;
use crate::authority::get_agent_activity_query::records::GetAgentActivityRecordsQuery;
use holo_hash::ActionHash;
use holo_hash::AgentPubKey;
use holochain_state::query::link::GetLinksQuery;
use holochain_state::query::Txn;
use holochain_state::query::{Query, Store};
use holochain_types::prelude::*;
use holochain_zome_types::agent_activity::DeterministicGetAgentActivityFilter;

#[cfg(test)]
mod test;

pub(crate) mod get_agent_activity_query;
pub(crate) mod get_entry_ops_query;
pub(crate) mod get_links_ops_query;
pub(crate) mod get_record_query;

/// Handler for get_entry query to an Entry authority
#[cfg_attr(feature = "instrument", tracing::instrument(skip(db)))]
pub async fn handle_get_entry(
    db: DbRead<DbKindDht>,
    hash: EntryHash,
    _options: holochain_p2p::event::GetOptions,
) -> CascadeResult<WireEntryOps> {
    let query = GetEntryOpsQuery::new(hash);
    let results = db.read_async(move |txn| query.run(Txn::from(&txn))).await?;
    Ok(results)
}

/// Handler for get_record query to a Record authority
#[cfg_attr(feature = "instrument", tracing::instrument(skip(env)))]
pub async fn handle_get_record(
    env: DbRead<DbKindDht>,
    hash: ActionHash,
    options: holochain_p2p::event::GetOptions,
) -> CascadeResult<WireRecordOps> {
    let query = GetRecordOpsQuery::new(hash, options);
    let results = env
        .read_async(move |txn| query.run(Txn::from(&txn)))
        .await?;
    Ok(results)
}

/// Handler for get_agent_activity query to an Activity authority.
#[cfg_attr(feature = "instrument", tracing::instrument(skip(env)))]
pub async fn handle_get_agent_activity(
    env: DbRead<DbKindDht>,
    agent: AgentPubKey,
    query: ChainQueryFilter,
    options: holochain_p2p::event::GetActivityOptions,
) -> CascadeResult<AgentActivityResponse> {
    let results = env
        .read_async(move |txn| -> CascadeResult<AgentActivityResponse> {
            let txn = Txn::from(&txn);

            let warrants =
                txn.get_warrants_for_basis(&AnyLinkableHash::from(agent.clone()), true)?;

            let mut activity_response = if options.include_full_records {
                // If the caller wanted records, prioritise giving those back.
                GetAgentActivityRecordsQuery::new(agent, query, options).run(txn)?
            } else if options.include_full_actions {
                // Otherwise, if the caller requested actions, give those back.
                GetAgentActivityActionsQuery::new(agent, query, options).run(txn)?
            } else {
                // Otherwise, just give back the hashes.
                GetAgentActivityHashesQuery::new(agent, query, options).run(txn)?
            };

            tracing::info!("Got activity response: {:?}", activity_response);

            if !warrants.is_empty() {
                // TODO why did we retrieve warrants in the activity query if we're going to overwrite them here?
                activity_response.warrants = warrants.into_iter().map(|w| w.into_warrant()).collect();
            }

            Ok(activity_response)
        })
        .await?;

    Ok(results)
}

/// Handler for must_get_agent_activity query to an Activity authority
#[cfg_attr(feature = "instrument", tracing::instrument(skip(env)))]
pub async fn handle_must_get_agent_activity(
    env: DbRead<DbKindDht>,
    author: AgentPubKey,
    filter: ChainFilter,
) -> CascadeResult<MustGetAgentActivityResponse> {
    Ok(must_get_agent_activity(env, author, filter).await?)
}

/// Handler for get_agent_activity_deterministic query to an Activity authority
#[cfg_attr(feature = "instrument", tracing::instrument(skip(env)))]
pub async fn handle_get_agent_activity_deterministic(
    env: DbRead<DbKindDht>,
    agent: AgentPubKey,
    filter: DeterministicGetAgentActivityFilter,
    options: holochain_p2p::event::GetActivityOptions,
) -> CascadeResult<DeterministicGetAgentActivityResponse> {
    let query = DeterministicGetAgentActivityQuery::new(agent, filter, options);
    let results = env
        .read_async(move |txn| query.run(Txn::from(&txn)))
        .await?;
    Ok(results)
}

/// Handler for get_links query to a Record/Entry authority
#[cfg_attr(feature = "instrument", tracing::instrument(skip(env, _options)))]
pub async fn handle_get_links(
    env: DbRead<DbKindDht>,
    link_key: WireLinkKey,
    _options: holochain_p2p::event::GetLinksOptions,
) -> CascadeResult<WireLinkOps> {
    let query = GetLinksOpsQuery::new(link_key);
    let results = env
        .read_async(move |txn| query.run(Txn::from(&txn)))
        .await?;
    Ok(results)
}

/// Handler for querying links
#[cfg_attr(feature = "instrument", tracing::instrument(skip(db)))]
pub async fn handle_get_links_query(
    db: DbRead<DbKindDht>,
    query: WireLinkQuery,
) -> CascadeResult<Vec<Link>> {
    let get_links_query = GetLinksQuery::new(
        query.base.clone(),
        query.link_type.clone(),
        query.tag_prefix.clone(),
        query.into(),
    );
    Ok(db
        .read_async(move |txn| get_links_query.run(Txn::from(&txn)))
        .await?)
}
