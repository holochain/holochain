use super::error::AuthorityDataError;
use super::error::CellResult;
use crate::conductor::CellError;
use crate::core::state::element_buf::ElementBuf;
use crate::core::state::metadata::ChainItemKey;
use crate::core::state::metadata::MetadataBuf;
use crate::core::state::metadata::MetadataBufT;
use fallible_iterator::FallibleIterator;

use holo_hash::AgentPubKey;
use holo_hash::EntryHash;
use holo_hash::HeaderHash;
use holochain_state::env::EnvironmentRead;
use holochain_state::env::EnvironmentWrite;
use holochain_state::env::ReadManager;
use holochain_state::error::DatabaseError;
use holochain_state::fresh_reader;
use holochain_state::prelude::PrefixType;
use holochain_state::prelude::Readable;
use holochain_types::activity::AgentActivity;
use holochain_types::activity::ChainItems;
use holochain_types::element::ElementStatus;
use holochain_types::element::GetElementResponse;
use holochain_types::element::RawGetEntryResponse;
use holochain_types::element::WireElement;
use holochain_types::header::WireHeaderStatus;
use holochain_types::header::WireUpdateRelationship;
use holochain_types::metadata::TimedHeaderHash;
use holochain_zome_types::element::SignedHeaderHashed;
use holochain_zome_types::header::conversions::WrongHeaderError;
use holochain_zome_types::query::ChainQueryFilter;
use holochain_zome_types::query::ChainStatus;
use holochain_zome_types::validate::ValidationStatus;
use std::collections::BTreeSet;
use std::convert::TryInto;
use tracing::*;

#[instrument(skip(state_env))]
pub async fn handle_get_entry(
    state_env: EnvironmentWrite,
    hash: EntryHash,
    options: holochain_p2p::event::GetOptions,
) -> CellResult<GetElementResponse> {
    // Get the vaults
    let element_vault = ElementBuf::vault(state_env.clone().into(), false)?;
    let element_rejected = ElementBuf::rejected(state_env.clone().into())?;
    let meta_vault = MetadataBuf::vault(state_env.clone().into())?;

    // ## Helper closures to DRY and make more readable

    // ### Render headers closure
    // Render headers from TimedHeaderHash to SignedHeaderHash
    let render_header_and_status = |timed_header_hash: TimedHeaderHash| {
        let header_hash = timed_header_hash.header_hash;
        let mut status = ValidationStatus::Valid;
        let mut r = element_vault.get_header(&header_hash)?;
        if r.is_none() {
            r = element_rejected.get_header(&header_hash)?;
            status = ValidationStatus::Rejected;
        }
        let r = r.ok_or_else(|| AuthorityDataError::missing_data(header_hash))?;
        CellResult::Ok((r, status))
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
            .get_all_headers(&reader, hash.clone())?
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
                let header_status = render_header_and_status(hash)?;
                live_headers.insert(header_status.try_into()?);
            }
            let updates_returns = meta_vault
                .get_updates(&reader, hash.clone().into())?
                .collect::<Vec<_>>()?;
            let updates_returns = updates_returns.into_iter().map(|update| {
                let update: WireHeaderStatus<WireUpdateRelationship> =
                    render_header_and_status(update)?
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
                    let header = render_header_and_status(hash)?;
                    live_headers.insert(header.try_into()?);
                }
            }
        }

        let mut return_deletes = Vec::with_capacity(deletes.len());
        for delete in deletes {
            let header = render_header_and_status(delete?)?;
            return_deletes.push(header.try_into().map_err(AuthorityDataError::from)?);
        }
        CellResult::Ok((live_headers, return_deletes, updates))
    };

    // ## Gather the entry and header data to return

    // ### Gather the entry
    // Get the entry from the first header

    fresh_reader!(state_env, |reader| {
        let first_header = meta_vault.get_all_headers(&reader, hash.clone())?.next()?;
        let entry_data = match first_header {
            Some(first_header) => {
                let header = render_header_and_status(first_header)?.0;
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

#[tracing::instrument(skip(env))]
pub async fn handle_get_element(
    env: EnvironmentWrite,
    hash: HeaderHash,
) -> CellResult<GetElementResponse> {
    // Get the vaults
    let env_ref = env.guard();
    let reader = env_ref.reader()?;
    let element_vault = ElementBuf::vault(env.clone().into(), false)?;
    let meta_vault = MetadataBuf::vault(env.clone().into())?;
    let element_rejected = ElementBuf::rejected(env.clone().into())?;

    // Check that we have the authority to serve this request because we have
    // done the StoreElement validation
    if !meta_vault.has_any_registered_store_element(&hash)? {
        return Ok(GetElementResponse::GetHeader(None));
    }

    // Look for a deletes on the header and collect them
    let deletes = meta_vault
        .get_deletes_on_header(&reader, hash.clone())?
        .map_err(CellError::from)
        .map(|delete_header| {
            let delete_hash = delete_header.header_hash;
            let mut status = ValidationStatus::Valid;
            let mut delete = element_vault.get_header(&delete_hash)?;
            if delete.is_none() {
                delete = element_rejected.get_header(&delete_hash)?;
                status = ValidationStatus::Rejected;
            }
            match delete {
                Some(delete) => Ok((delete, status)
                    .try_into()
                    .map_err(AuthorityDataError::from)?),
                None => Err(AuthorityDataError::missing_data(delete)),
            }
        })
        .collect()?;

    // Look for a updates on the header and collect them
    let updates = meta_vault
        .get_updates(&reader, hash.clone().into())?
        .map_err(CellError::from)
        .map(|update_header| {
            let update_hash = update_header.header_hash;
            let mut status = ValidationStatus::Valid;
            let mut update = element_vault.get_header(&update_hash)?;
            if update.is_none() {
                update = element_rejected.get_header(&update_hash)?;
                status = ValidationStatus::Rejected;
            }
            match update {
                Some(update) => Ok((update, status)
                    .try_into()
                    .map_err(AuthorityDataError::from)?),
                None => Err(AuthorityDataError::missing_data(update)),
            }
        })
        .collect()?;

    // Get the actual header and return it with proof of deleted if there is any
    let mut r = element_vault.get_element(&hash)?;
    let mut status = ValidationStatus::Valid;
    if r.is_none() {
        r = element_rejected.get_element(&hash)?;
        status = ValidationStatus::Rejected;
    }
    let r = r
        .map(|e| WireElement::from_element(ElementStatus::new(e, status), deletes, updates))
        .map(Box::new);

    Ok(GetElementResponse::GetHeader(r))
}

#[instrument(skip(env))]
pub fn handle_get_agent_activity(
    env: EnvironmentRead,
    agent: AgentPubKey,
    query: ChainQueryFilter,
    options: holochain_p2p::event::GetActivityOptions,
) -> CellResult<AgentActivity> {
    // Databases
    let element_integrated = ElementBuf::vault(env.clone(), false)?;
    let meta_integrated = MetadataBuf::vault(env.clone())?;
    let element_rejected = ElementBuf::rejected(env.clone())?;

    // Status
    let status = meta_integrated
        .get_activity_status(&agent)?
        .unwrap_or(ChainStatus::Empty);
    let highest_observed = meta_integrated.get_activity_observed(&agent)?;

    // Valid headers
    let valid_activity = if options.include_valid_activity {
        fresh_reader!(env, |r| {
            let hashes = meta_integrated.get_activity_sequence(
                &r,
                ChainItemKey::AgentStatus(agent.clone(), ValidationStatus::Valid),
            )?;
            check_headers(
                hashes,
                query.clone(),
                options.clone(),
                element_integrated,
                &r,
            )
        })?
    } else {
        ChainItems::NotRequested
    };

    // Rejected hashes
    let rejected_activity = if options.include_rejected_activity {
        fresh_reader!(env, |r| {
            let hashes = meta_integrated.get_activity_sequence(
                &r,
                ChainItemKey::AgentStatus(agent.clone(), ValidationStatus::Rejected),
            )?;
            check_headers(hashes, query, options, element_rejected, &r)
        })?
    } else {
        ChainItems::NotRequested
    };

    Ok(AgentActivity {
        valid_activity,
        rejected_activity,
        agent,
        status,
        highest_observed,
    })
}

fn get_full_headers<'a, P: PrefixType + 'a, R: Readable>(
    hashes: impl FallibleIterator<Item = (u32, HeaderHash), Error = DatabaseError> + 'a,
    query: ChainQueryFilter,
    database: ElementBuf<P>,
    reader: &'a R,
) -> impl FallibleIterator<Item = (u32, SignedHeaderHashed), Error = DatabaseError> + 'a {
    hashes
        .filter_map(move |(s, h)| {
            Ok(database
                .get_header_with_reader(reader, &h)?
                .map(|shh| (s, shh)))
        })
        .filter(move |(_, shh)| Ok(query.check(shh.header())))
}

fn check_headers<P: PrefixType, R: Readable>(
    hashes: impl FallibleIterator<Item = (u32, HeaderHash), Error = DatabaseError>,
    query: ChainQueryFilter,
    options: holochain_p2p::event::GetActivityOptions,
    database: ElementBuf<P>,
    reader: &R,
) -> CellResult<ChainItems> {
    if options.include_full_headers {
        CellResult::Ok(ChainItems::Full(
            get_full_headers(hashes, query, database, reader)
                .map(|(_, shh)| Ok(shh))
                .collect()?,
        ))
    } else {
        Ok(ChainItems::Hashes(
            get_full_headers(hashes, query, database, reader)
                .map(|(s, shh)| Ok((s, shh.into_inner().1)))
                .collect()?,
        ))
    }
}

#[cfg(test)]
#[instrument(skip(env))]
// This is handy for testing performance as it shows the read times for get agent activity
fn _show_agent_activity_read_times(env: EnvironmentRead, agent: AgentPubKey) {
    {
        let g = env.guard();
        let rkv = g.rkv();
        let stat = rkv.stat().unwrap();
        let info = rkv.info().unwrap();
        debug!(
            map_size = info.map_size(),
            last_pgno = info.last_pgno(),
            last_txnid = info.last_txnid(),
            max_readers = info.max_readers(),
            num_readers = info.num_readers()
        );
        debug!(
            page_size = stat.page_size(),
            depth = stat.depth(),
            branch_pages = stat.branch_pages(),
            leaf_pages = stat.leaf_pages(),
            overflow_pages = stat.overflow_pages(),
            entries = stat.entries(),
        );
    }
    let element_integrated = ElementBuf::vault(env.clone(), false).unwrap();
    let meta_integrated = MetadataBuf::vault(env.clone()).unwrap();
    holochain_state::fresh_reader_test!(env, |r| {
        let now = std::time::Instant::now();
        let hashes = meta_integrated
            .get_activity_sequence(
                &r,
                ChainItemKey::AgentStatus(agent.clone(), ValidationStatus::Valid),
            )
            .unwrap()
            .collect::<Vec<_>>()
            .unwrap();
        let el = now.elapsed();
        debug!(time_for_activity_sequence = %el.as_micros());
        for hash in &hashes {
            element_integrated.get_header(&hash.1).unwrap();
        }
        let el = now.elapsed();
        debug!(
            us_per_header = %el.as_micros() / hashes.len() as u128,
            num_headers = %hashes.len(),
            total = %el.as_millis()
        );
    });
}
