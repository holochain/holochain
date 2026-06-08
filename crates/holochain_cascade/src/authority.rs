//! Functions for the various authorities to handle queries.
//!
//! The get authorities serve **records** — actions plus their entry — each
//! carrying its record-level validation status. A `Rejected` record is always
//! paired with a warrant proving the rejection, so the receiver can reject it
//! up front without being forced into pointless validation work. All reads go
//! through the `holochain_data`-backed [`DhtStoreRead`]; only locally-validated
//! data is served (enforced by the store's authority reads).

use super::error::CascadeResult;
use holo_hash::ActionHash;
use holo_hash::AgentPubKey;
use holo_hash::EntryHash;
use holochain_state::dht_store::{DhtStoreRead, GetAgentActivityOptions};
use holochain_state::query::link::GetLinksFilter;
use holochain_types::prelude::*;
use holochain_zome_types::warrant::SignedWarrant;
use std::collections::HashSet;

pub(crate) mod get_agent_activity_query;

/// Handler for get_entry query to an Entry authority.
#[cfg_attr(feature = "instrument", tracing::instrument(skip(store)))]
pub async fn handle_get_entry(store: DhtStoreRead, hash: EntryHash) -> CascadeResult<WireEntryOps> {
    let mut rejected_authors = HashSet::new();

    let create_rows = store.get_authority_entry_creates(&hash).await?;
    let delete_rows = store.get_authority_deletes_for_entry(&hash).await?;
    let update_rows = store.get_authority_updates_for_entry(&hash).await?;

    // The entry type is shared across all actions on the entry; take it from
    // whichever create or update action serves it.
    let entry_type = create_rows
        .iter()
        .chain(update_rows.iter())
        .find_map(|(sah, _)| sah.action().entry_type().cloned());

    let creates = judged_actions(create_rows, &mut rejected_authors);
    let deletes = judged_actions(delete_rows, &mut rejected_authors);
    let updates = judged_actions(update_rows, &mut rejected_authors);

    let entry = match store.retrieve_entry(&hash, None).await? {
        Some(entry) => entry_type.map(|entry_type| EntryData { entry, entry_type }),
        None => None,
    };

    let warrants = collect_warrants(&store, rejected_authors).await?;

    Ok(WireEntryOps {
        creates,
        deletes,
        updates,
        entry,
        warrants,
    })
}

/// Handler for get_record query to a Record authority.
#[cfg_attr(feature = "instrument", tracing::instrument(skip(store)))]
pub async fn handle_get_record(
    store: DhtStoreRead,
    hash: ActionHash,
) -> CascadeResult<WireRecordOps> {
    let mut rejected_authors = HashSet::new();

    let mut entry = None;
    let action = match store.get_authority_store_record(&hash).await? {
        Some((sah, status)) => {
            if status == ValidationStatus::Rejected {
                rejected_authors.insert(sah.action().author().clone());
            }
            if let Some(entry_hash) = sah.action().entry_hash().cloned() {
                entry = store.retrieve_entry(&entry_hash, None).await?;
            }
            Some(Judged::new(SignedAction::from(sah), status))
        }
        None => None,
    };

    let deletes = judged_actions(
        store.get_authority_deletes_for_record(&hash).await?,
        &mut rejected_authors,
    );
    let updates = judged_actions(
        store.get_authority_updates_for_record(&hash).await?,
        &mut rejected_authors,
    );

    let warrants = collect_warrants(&store, rejected_authors).await?;

    Ok(WireRecordOps {
        action,
        deletes,
        updates,
        entry,
        warrants,
    })
}

/// Handler for get_agent_activity query to an Activity authority.
#[cfg_attr(feature = "instrument", tracing::instrument(skip(store)))]
pub async fn handle_get_agent_activity(
    store: DhtStoreRead,
    agent: AgentPubKey,
    query: ChainQueryFilter,
    options: holochain_p2p::event::GetActivityOptions,
) -> CascadeResult<AgentActivityResponse> {
    let options = GetAgentActivityOptions {
        include_valid_activity: options.include_valid_activity,
        include_rejected_activity: options.include_rejected_activity,
        include_warrants: options.include_warrants,
        include_full_records: options.include_full_records,
    };
    Ok(store.get_agent_activity(&agent, &query, &options).await?)
}

/// Handler for must_get_agent_activity query to an Activity authority.
#[cfg_attr(feature = "instrument", tracing::instrument(skip(store)))]
pub async fn handle_must_get_agent_activity(
    store: DhtStoreRead,
    author: AgentPubKey,
    filter: ChainFilter,
) -> CascadeResult<MustGetAgentActivityResponse> {
    Ok(store.must_get_agent_activity(&author, &filter).await?)
}

/// Handler for get_links query to a Record/Entry authority.
#[cfg_attr(feature = "instrument", tracing::instrument(skip(store, _options)))]
pub async fn handle_get_links(
    store: DhtStoreRead,
    link_key: WireLinkKey,
    _options: holochain_p2p::event::GetLinksOptions,
) -> CascadeResult<WireLinkOps> {
    let mut rejected_authors = HashSet::new();

    let create_rows = store.get_authority_link_creates(&link_key.base).await?;
    let create_rows = filter_link_creates(create_rows, &link_key);
    let delete_rows = store.get_authority_delete_links(&link_key.base).await?;

    let creates = judged_actions(create_rows, &mut rejected_authors);
    let deletes = judged_actions(delete_rows, &mut rejected_authors);

    let warrants = collect_warrants(&store, rejected_authors).await?;

    Ok(WireLinkOps {
        creates,
        deletes,
        warrants,
    })
}

/// Handler for querying links (returns rendered [`Link`]s, not wire ops).
#[cfg_attr(feature = "instrument", tracing::instrument(skip(store)))]
pub async fn handle_get_links_query(
    store: DhtStoreRead,
    query: WireLinkQuery,
) -> CascadeResult<Vec<Link>> {
    let filter = GetLinksFilter {
        after: query.after,
        before: query.before,
        author: query.author.clone(),
    };
    Ok(store
        .get_links(
            &query.base,
            &query.link_type,
            query.tag_prefix.as_ref(),
            &filter,
        )
        .await?)
}

/// Convert authority-read rows into wire-ready judged actions, recording the
/// authors of any `Rejected` records so their warrants can be attached.
fn judged_actions(
    rows: Vec<(SignedActionHashed, ValidationStatus)>,
    rejected_authors: &mut HashSet<AgentPubKey>,
) -> Vec<Judged<SignedAction>> {
    rows.into_iter()
        .map(|(sah, status)| {
            if status == ValidationStatus::Rejected {
                rejected_authors.insert(sah.action().author().clone());
            }
            Judged::new(SignedAction::from(sah), status)
        })
        .collect()
}

/// Fetch the warrants proving the rejection of every `Rejected` record served,
/// keyed by the rejected record's author (the warrantee).
async fn collect_warrants(
    store: &DhtStoreRead,
    authors: HashSet<AgentPubKey>,
) -> CascadeResult<Vec<SignedWarrant>> {
    let mut warrants = Vec::new();
    for author in authors {
        warrants.extend(store.get_warrants_by_warrantee(author).await?);
    }
    Ok(warrants)
}

/// Apply the wire link key's type/tag/author/time filters to create-link rows.
fn filter_link_creates(
    rows: Vec<(SignedActionHashed, ValidationStatus)>,
    key: &WireLinkKey,
) -> Vec<(SignedActionHashed, ValidationStatus)> {
    rows.into_iter()
        .filter(|(sah, _)| {
            let action = sah.action();
            let Action::CreateLink(create_link) = action else {
                return false;
            };
            if !key
                .type_query
                .contains(&create_link.zome_index, &create_link.link_type)
            {
                return false;
            }
            if let Some(tag) = &key.tag {
                if !create_link.tag.0.starts_with(&tag.0) {
                    return false;
                }
            }
            if let Some(author) = &key.author {
                if action.author() != author {
                    return false;
                }
            }
            if let Some(before) = key.before {
                if action.timestamp() > before {
                    return false;
                }
            }
            if let Some(after) = key.after {
                if action.timestamp() < after {
                    return false;
                }
            }
            true
        })
        .collect()
}
