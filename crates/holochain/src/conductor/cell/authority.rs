use super::error::{AuthorityDataError, CellResult};
use crate::core::state::{
    element_buf::ElementBuf,
    metadata::{MetadataBuf, MetadataBufT},
};
use fallible_iterator::FallibleIterator;

use holo_hash::EntryHash;
use holochain_state::{env::EnvironmentWrite, fresh_reader};
use holochain_types::{
    element::{GetElementResponse, RawGetEntryResponse},
    header::WireEntryUpdateRelationship,
    metadata::TimedHeaderHash,
};
use holochain_zome_types::{element::SignedHeaderHashed, header::conversions::WrongHeaderError};
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
    let gather_headers = || {
        let mut deletes = Vec::new();
        let mut updates = Vec::new();
        let headers = fresh_reader!(meta_vault.env(), |r| meta_vault
            .get_headers(&r, hash.clone())?
            .collect::<Vec<_>>())?;
        let mut live_headers = BTreeSet::new();

        // We want all the live headers and deletes
        if options.all_live_headers_with_metadata {
            for hash in headers {
                fresh_reader!(meta_vault.env(), |r| {
                    deletes.extend(
                        meta_vault
                            .get_deletes_on_header(&r, hash.header_hash.clone())?
                            .iterator(),
                    );
                    CellResult::Ok(())
                })?;
                let header = render_header(hash)?;
                live_headers.insert(header.try_into()?);
            }
            let updates_returns = fresh_reader!(meta_vault.env(), |r| meta_vault
                .get_updates(&r, hash.clone().into())?
                .collect::<Vec<_>>())?;
            let updates_returns = updates_returns.into_iter().map(|update| {
                let update: WireEntryUpdateRelationship = render_header(update)?
                    .try_into()
                    .map_err(AuthorityDataError::from)?;
                CellResult::Ok(update)
            });
            updates = updates_returns.collect::<Result<_, _>>()?;

        // We only want the headers if they are live and all deletes
        } else {
            for hash in headers {
                // Check for a delete
                let is_deleted = fresh_reader!(meta_vault.env(), |r| CellResult::Ok(
                    meta_vault
                        .get_deletes_on_header(&r, hash.header_hash.clone())?
                        .next()?
                        .is_some()
                ))?;

                // If there is a delete then gather all deletes
                if is_deleted {
                    fresh_reader!(meta_vault.env(), |r| {
                        deletes.extend(
                            meta_vault
                                .get_deletes_on_header(&r, hash.header_hash.clone())?
                                .iterator(),
                        );
                        CellResult::Ok(())
                    })?;

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

    let first_header = fresh_reader!(state_env, |reader| {
        meta_vault.get_headers(&reader, hash.clone())?.next()
    })?;
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
            let (live_headers, deletes, updates) = gather_headers()?;
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
}
