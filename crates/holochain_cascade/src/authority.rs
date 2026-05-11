//! Functions for the various authorities to handle queries

use self::get_entry_ops_query::GetEntryOpsQuery;
use self::get_links_ops_query::GetLinksOpsQuery;
use self::get_record_query::GetRecordOpsQuery;
use super::error::CascadeResult;
use crate::authority::get_agent_activity_query::hashes::GetAgentActivityHashesQuery;
use crate::authority::get_agent_activity_query::records::GetAgentActivityRecordsQuery;
use crate::CascadeImpl;
use holo_hash::ActionHash;
use holo_hash::AgentPubKey;
use holochain_p2p::actor::NetworkRequestOptions;
use holochain_state::query::link::GetLinksQuery;
use holochain_state::query::CascadeTxnWrapper;
use holochain_state::query::{Query, Store};
use holochain_types::prelude::*;

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
) -> CascadeResult<WireEntryOps> {
    let query = GetEntryOpsQuery::new(hash);
    let results = db
        .read_async(move |txn| query.run(CascadeTxnWrapper::from(txn)))
        .await?;
    Ok(results)
}

/// Handler for get_record query to a Record authority
#[cfg_attr(feature = "instrument", tracing::instrument(skip(env)))]
pub async fn handle_get_record(
    env: DbRead<DbKindDht>,
    hash: ActionHash,
) -> CascadeResult<WireRecordOps> {
    let query = GetRecordOpsQuery::new(hash);
    let results = env
        .read_async(move |txn| query.run(CascadeTxnWrapper::from(txn)))
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
            let txn = CascadeTxnWrapper::from(txn);

            // The activity query only selects actions from the database. Warrants have
            // a different schema and must be fetched separately.
            let warrants = txn.get_warrants_for_agent(&agent, true)?;

            let mut activity_response = if options.include_full_records {
                // If the caller wanted records, prioritise giving those back.
                GetAgentActivityRecordsQuery::new(agent, query, options).run(txn)?
            } else {
                // Otherwise, just give back the hashes.
                GetAgentActivityHashesQuery::new(agent, query, options).run(txn)?
            };

            activity_response.warrants = warrants.into_iter().map(|w| (*w).clone()).collect();

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
    CascadeImpl::empty()
        .with_dht(env)
        .must_get_agent_activity(author, filter, NetworkRequestOptions::default())
        .await
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
        .read_async(move |txn| query.run(CascadeTxnWrapper::from(txn)))
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
        .read_async(move |txn| get_links_query.run(CascadeTxnWrapper::from(txn)))
        .await?)
}
