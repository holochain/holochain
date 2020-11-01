use super::error::{AuthorityDataError, CellResult};
use crate::core::state::{
    element_buf::ElementBuf,
    metadata::{ChainItemKey, MetadataBuf, MetadataBufT},
};
use fallible_iterator::FallibleIterator;

use holo_hash::{AgentPubKey, EntryHash, HeaderHash};
use holochain_state::{
    env::EnvironmentRead, env::EnvironmentWrite, fresh_reader, prelude::PrefixType,
    prelude::Readable,
};
use holochain_types::{
    element::{GetElementResponse, RawGetEntryResponse},
    header::WireUpdateRelationship,
    metadata::TimedHeaderHash,
};
use holochain_zome_types::{
    element::SignedHeaderHashed, header::conversions::WrongHeaderError, query::Activity,
    query::AgentActivity, query::ChainQueryFilter, query::ChainStatus,
};
use std::{collections::BTreeSet, convert::TryInto};
use tracing::*;

#[instrument(skip(state_env))]
pub async fn handle_get_entry(
    state_env: EnvironmentWrite,
    hash: EntryHash,
    options: holochain_p2p::event::GetOptions,
) -> CellResult<GetElementResponse> {
    // Get the vaults
    let element_vault = ElementBuf::vault(state_env.clone().into(), false)?;
    let meta_vault = MetadataBuf::vault(state_env.clone().into())?;

    // ## Helper closures to DRY and make more readable

    // ### Render headers closure
    // Render headers from TimedHeaderHash to SignedHeaderHash
    let render_header = |timed_header_hash: TimedHeaderHash| {
        let header_hash = timed_header_hash.header_hash;
        let r = element_vault
            .get_header(&header_hash)?
            .ok_or_else(|| AuthorityDataError::missing_data(header_hash))?;
        CellResult::Ok(r)
    };

    // ### Get entry data closure
    // Get the entry from a header
    let get_entry = |header: SignedHeaderHashed| {
        // Does the header contain entry data?
        let (eh, et) = header.header().entry_data().ok_or_else(|| {
            AuthorityDataError::WrongHeaderError(WrongHeaderError(format!(
                "Header should have entry data: {:?}",
                header
            )))
        })?;

        // Can we get the actual entry
        let entry_data = element_vault
            .get_entry(&eh)?
            .map(|e| (e.into_content(), et.clone()))
            // Missing the entry
            .ok_or_else(|| AuthorityDataError::missing_data_entry(header))?;
        CellResult::Ok(entry_data)
    };

    // ### Gather headers closure
    // This gathers the headers and deletes we want
    let gather_headers = |reader| {
        let mut deletes = Vec::new();
        let mut updates = Vec::new();
        let headers = meta_vault
            .get_headers(&reader, hash.clone())?
            .collect::<Vec<_>>()?;
        let mut live_headers = BTreeSet::new();

        // We want all the live headers and deletes
        if options.all_live_headers_with_metadata {
            for hash in headers {
                deletes.extend(
                    meta_vault
                        .get_deletes_on_header(&reader, hash.header_hash.clone())?
                        .iterator(),
                );
                let header = render_header(hash)?;
                live_headers.insert(header.try_into()?);
            }
            let updates_returns = meta_vault
                .get_updates(&reader, hash.clone().into())?
                .collect::<Vec<_>>()?;
            let updates_returns = updates_returns.into_iter().map(|update| {
                let update: WireUpdateRelationship = render_header(update)?
                    .try_into()
                    .map_err(AuthorityDataError::from)?;
                CellResult::Ok(update)
            });
            updates = updates_returns.collect::<Result<_, _>>()?;

        // We only want the headers if they are live and all deletes
        } else {
            for hash in headers {
                // Check for a delete
                let is_deleted = meta_vault
                    .get_deletes_on_header(&reader, hash.header_hash.clone())?
                    .next()?
                    .is_some();

                // If there is a delete then gather all deletes
                if is_deleted {
                    deletes.extend(
                        meta_vault
                            .get_deletes_on_header(&reader, hash.header_hash.clone())?
                            .iterator(),
                    );

                // Otherwise gather the header
                } else {
                    let header = render_header(hash)?;
                    live_headers.insert(header.try_into()?);
                }
            }
        }

        let mut return_deletes = Vec::with_capacity(deletes.len());
        for delete in deletes {
            let header = render_header(delete?)?;
            return_deletes.push(header.try_into().map_err(AuthorityDataError::from)?);
        }
        CellResult::Ok((live_headers, return_deletes, updates))
    };

    // ## Gather the entry and header data to return

    // ### Gather the entry
    // Get the entry from the first header

    fresh_reader!(state_env, |reader| {
        let first_header = meta_vault.get_headers(&reader, hash.clone())?.next()?;
        let entry_data = match first_header {
            Some(first_header) => {
                let header = render_header(first_header)?;
                Some(get_entry(header)?)
            }
            None => None,
        };

        let r = match entry_data {
            Some((entry, entry_type)) => {
                // ### Gather headers
                // There is at least one header with an entry so gather all the required data
                let (live_headers, deletes, updates) = gather_headers(reader)?;
                let r = RawGetEntryResponse {
                    live_headers,
                    deletes,
                    updates,
                    entry,
                    entry_type,
                };
                Some(Box::new(r))
            }
            _ => None,
        };
        debug!(handle_get_details_return = ?r);
        Ok(GetElementResponse::GetEntryFull(r))
    })
}

#[instrument(skip(env))]
pub fn handle_get_agent_activity(
    env: EnvironmentRead,
    agent: AgentPubKey,
    // TODO: Query filtering breaks caching.
    // It's easier to just send back the full chain and then filter
    // in the cascade but it would be nice to avoid sending the filtered
    // out headers across the network.
    _query: ChainQueryFilter,
    options: holochain_p2p::event::GetActivityOptions,
) -> CellResult<AgentActivity> {
    // Databases
    let element_integrated = ElementBuf::vault(env.clone(), false)?;
    let meta_integrated = MetadataBuf::vault(env.clone())?;
    let element_rejected = ElementBuf::rejected(env.clone())?;
    let meta_rejected = MetadataBuf::rejected(env.clone())?;

    // Status
    let status = meta_integrated
        .get_activity_status(&agent)?
        .unwrap_or(ChainStatus::Empty);
    let highest_observed = meta_integrated.get_activity_observed(&agent)?;

    // TODO: If full headers aren't requested then query doesn't work

    // Valid headers
    let valid_activity = if options.include_valid_activity {
        fresh_reader!(env, |r| {
            let hashes = meta_integrated
                .get_activity_sequence(&r, ChainItemKey::Agent(agent.clone()))?
                .collect()?;
            if options.include_full_headers {
                CellResult::Ok(Activity::Full(get_full_headers(
                    hashes,
                    element_integrated,
                    &r,
                )?))
            } else {
                Ok(Activity::Hashes(hashes))
            }
        })?
    } else {
        Activity::NotRequested
    };

    // Rejected hashes
    let rejected_activity = if options.include_rejected_activity {
        fresh_reader!(env, |r| {
            let hashes = meta_rejected
                .get_activity_sequence(&r, ChainItemKey::Agent(agent.clone()))?
                .collect()?;
            if options.include_full_headers {
                CellResult::Ok(Activity::Full(get_full_headers(
                    hashes,
                    element_rejected,
                    &r,
                )?))
            } else {
                Ok(Activity::Hashes(hashes))
            }
        })?
    } else {
        Activity::NotRequested
    };

    Ok(AgentActivity {
        valid_activity,
        rejected_activity,
        agent,
        status,
        highest_observed,
    })
}

fn get_full_headers<P: PrefixType, R: Readable>(
    hashes: Vec<(u32, HeaderHash)>,
    database: ElementBuf<P>,
    reader: &R,
) -> CellResult<Vec<SignedHeaderHashed>> {
    let headers: Vec<_> = fallible_iterator::convert(hashes.into_iter().map(Ok))
        .filter_map(|h| database.get_header_with_reader(reader, &h.1))
        // .filter(|shh| Ok(query.check(shh.header())))
        .collect()?;
    Ok(headers)
}
