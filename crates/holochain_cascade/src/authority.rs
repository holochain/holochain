use super::error::AuthorityDataError;
use super::error::CascadeError;
use super::error::CascadeResult;
use fallible_iterator::FallibleIterator;
use holo_hash::AgentPubKey;
use holo_hash::EntryHash;
use holo_hash::HeaderHash;
use holochain_sqlite::fresh_reader;
use holochain_sqlite::prelude::*;
use holochain_state::element_buf::ElementBuf;
use holochain_state::metadata::ChainItemKey;
use holochain_state::metadata::LinkMetaKey;
use holochain_state::metadata::MetadataBuf;
use holochain_state::metadata::MetadataBufT;
use holochain_types::prelude::*;
use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::convert::TryInto;
use tracing::*;

#[instrument(skip(state_env))]
pub fn handle_get_entry(
    state_env: EnvRead,
    hash: EntryHash,
    options: holochain_p2p::event::GetOptions,
) -> CascadeResult<GetElementResponse> {
    // Get the vaults
    let element_vault = ElementBuf::vault(state_env.clone(), false)?;
    let element_rejected = ElementBuf::rejected(state_env.clone())?;
    let meta_vault = MetadataBuf::vault(state_env.clone())?;

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
        CascadeResult::Ok((r, status))
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
        CascadeResult::Ok(entry_data)
    };

    // ### Gather headers closure
    // This gathers the headers and deletes we want
    let gather_headers = |mut reader: Reader| {
        let mut deletes = Vec::new();
        let mut updates = Vec::new();
        let headers = meta_vault
            .get_all_headers(&mut reader, hash.clone())?
            .collect::<Vec<_>>()?;
        let mut live_headers = BTreeSet::new();

        // We want all the live headers and deletes
        if options.all_live_headers_with_metadata {
            for hash in headers {
                deletes.extend(
                    meta_vault
                        .get_deletes_on_header(&mut reader, hash.header_hash.clone())?
                        .iterator(),
                );
                let header_status = render_header_and_status(hash)?;
                live_headers.insert(header_status.try_into()?);
            }
            let updates_returns = meta_vault
                .get_updates(&mut reader, hash.clone().into())?
                .collect::<Vec<_>>()?;
            let updates_returns = updates_returns.into_iter().map(|update| {
                let update: WireHeaderStatus<WireUpdateRelationship> =
                    render_header_and_status(update)?
                        .try_into()
                        .map_err(AuthorityDataError::from)?;
                CascadeResult::Ok(update)
            });
            updates = updates_returns.collect::<Result<_, _>>()?;

        // We only want the headers if they are live and all deletes
        } else {
            for hash in headers {
                // Check for a delete
                let is_deleted = meta_vault
                    .get_deletes_on_header(&mut reader, hash.header_hash.clone())?
                    .next()?
                    .is_some();

                // If there is a delete then gather all deletes
                if is_deleted {
                    deletes.extend(
                        meta_vault
                            .get_deletes_on_header(&mut reader, hash.header_hash.clone())?
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
        CascadeResult::Ok((live_headers, return_deletes, updates))
    };

    // ## Gather the entry and header data to return

    // ### Gather the entry
    // Get the entry from the first header

    let entry_data = fresh_reader!(state_env, |mut reader| {
        let first_header = meta_vault
            .get_all_headers(&mut reader, hash.clone())?
            .next()?;
        let entry_data = match first_header {
            Some(first_header) => {
                let header = render_header_and_status(first_header)?.0;
                Some(get_entry(header)?)
            }
            None => None,
        };
        CascadeResult::Ok(entry_data)
    })?;

    let r = match entry_data {
        Some((entry, entry_type)) => {
            // ### Gather headers
            // There is at least one header with an entry so gather all the required data
            let (live_headers, deletes, updates) = fresh_reader!(state_env, gather_headers)?;
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

#[tracing::instrument(skip(env))]
pub fn handle_get_element(env: EnvRead, hash: HeaderHash) -> CascadeResult<GetElementResponse> {
    // Get the vaults
    let element_vault = ElementBuf::vault(env.clone(), false)?;
    let meta_vault = MetadataBuf::vault(env.clone())?;
    let element_rejected = ElementBuf::rejected(env.clone())?;

    // Check that we have the authority to serve this request because we have
    // done the StoreElement validation
    if !meta_vault.has_any_registered_store_element(&hash)? {
        return Ok(GetElementResponse::GetHeader(None));
    }
    let mut conn = env.conn()?;
    conn.with_reader(|mut reader| {
        // Look for a deletes on the header and collect them
        let deletes = meta_vault
            .get_deletes_on_header(&mut reader, hash.clone())?
            .map_err(CascadeError::from)
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
            .get_updates(&mut reader, hash.clone().into())?
            .map_err(CascadeError::from)
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
    })
}

#[instrument(skip(env))]
pub fn handle_get_agent_activity(
    env: EnvRead,
    agent: AgentPubKey,
    query: ChainQueryFilter,
    options: holochain_p2p::event::GetActivityOptions,
) -> CascadeResult<AgentActivityResponse> {
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
        fresh_reader!(env, |mut r| {
            let hashes = meta_integrated.get_activity_sequence(
                &mut r,
                ChainItemKey::AgentStatus(agent.clone(), ValidationStatus::Valid),
            )?;
            check_headers(
                hashes,
                query.clone(),
                options.clone(),
                element_integrated,
                &mut r,
            )
        })?
    } else {
        ChainItems::NotRequested
    };

    // Rejected hashes
    let rejected_activity = if options.include_rejected_activity {
        fresh_reader!(env, |mut r| {
            let hashes = meta_integrated.get_activity_sequence(
                &mut r,
                ChainItemKey::AgentStatus(agent.clone(), ValidationStatus::Rejected),
            )?;
            check_headers(hashes, query, options, element_rejected, &mut r)
        })?
    } else {
        ChainItems::NotRequested
    };

    Ok(AgentActivityResponse {
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
    reader: &'a mut R, // maybe not 'a
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
    reader: &mut R,
) -> CascadeResult<ChainItems> {
    if options.include_full_headers {
        CascadeResult::Ok(ChainItems::Full(
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

#[instrument(skip(env, _options))]
pub fn handle_get_links(
    env: EnvRead,
    link_key: WireLinkMetaKey,
    _options: holochain_p2p::event::GetLinksOptions,
) -> CascadeResult<GetLinksResponse> {
    // Get the vaults
    let mut conn = env.conn()?;
    let element_vault = ElementBuf::vault(env.clone(), false)?;
    let meta_vault = MetadataBuf::vault(env)?;

    let links = conn.with_reader(|mut reader| {
        meta_vault
            .get_links_all(&mut reader, &LinkMetaKey::from(&link_key))?
            .map(|link_add| {
                // Collect the link removes on this link add
                let link_removes = meta_vault
                    .get_link_removes_on_link_add(&mut reader, link_add.link_add_hash.clone())?
                    .collect::<BTreeSet<_>>()?;
                // Create timed header hash
                let link_add = TimedHeaderHash {
                    timestamp: link_add.timestamp,
                    header_hash: link_add.link_add_hash,
                };
                // Return all link removes with this link add
                Ok((link_add, link_removes))
            })
            .collect::<BTreeMap<_, _>>()
    })?;

    // Get the headers from the element stores
    let mut result_adds: Vec<(CreateLink, Signature)> = Vec::with_capacity(links.len());
    let mut result_removes: Vec<(DeleteLink, Signature)> = Vec::with_capacity(links.len());
    for (link_add, link_removes) in links {
        if let Some(link_add) = element_vault.get_header(&link_add.header_hash)? {
            for link_remove in link_removes {
                if let Some(link_remove) = element_vault.get_header(&link_remove.header_hash)? {
                    let (h, s) = link_remove.into_header_and_signature();
                    let h = h
                        .into_content()
                        .try_into()
                        .map_err(AuthorityDataError::from)?;
                    result_removes.push((h, s));
                }
            }
            let (h, s) = link_add.into_header_and_signature();
            let h = h
                .into_content()
                .try_into()
                .map_err(AuthorityDataError::from)?;
            result_adds.push((h, s));
        }
    }

    // Return the links
    Ok(GetLinksResponse {
        link_adds: result_adds,
        link_removes: result_removes,
    })
}
