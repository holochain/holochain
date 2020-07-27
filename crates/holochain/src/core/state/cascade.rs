//! # Cascade
//! This module is still a work in progress.
//! Here is some pseudocode we are using to build it.
//! ## Dimensions
//! get vs get_links
//! default vs options
//! fast vs strict (is set by app dev)
//!
//! ## Get
//! ### Default - Get's the latest version
//! Scratch Live -> Return
//! Scratch NotInCascade -> Goto Cas
//! Scratch _ -> None
//! Cas Live -> Return
//! Cas NotInCascade -> Goto cache
//! Cas _ -> None
//! Cache Live -> Return
//! Cache Pending -> Goto Network
//! Cache NotInCascade -> Goto Network
//! Cache _ -> None
//!
//! ## Get Links
//! ### Default - Get's the latest version
//! if I'm an authority
//! Scratch Found-> Return
//! Scratch NotInCascade -> Goto Cas
//! Cas Found -> Return
//! Cas NotInCascade -> Goto Network
//! else
//! Network Found -> Return
//! Network NotInCascade -> Goto Cache
//! Cache Found -> Return
//! Cache NotInCascade -> None
//!
//! ## Pagination
//! gets most recent N links with default N (50)
//! Page number
//! ## Loading
//! load_true loads the results into cache

use super::{
    chain_cas::ChainCasBuf,
    metadata::{LinkMetaKey, MetadataBuf, MetadataBufT, SysMetaVal},
};
use error::CascadeResult;
use fallible_iterator::FallibleIterator;
use holo_hash::{
    hash_type::{self, AnyDht},
    AnyDhtHash, EntryHash, HeaderHash,
};
use holochain_keystore::Signature;
use holochain_p2p::{
    actor::{GetMetaOptions, GetOptions},
    HolochainP2pCell,
};
use holochain_state::error::DatabaseResult;
use holochain_types::{
    element::{
        ChainElement, GetElementResponse, RawGetEntryResponse, SignedHeaderHashed, WireElement,
    },
    header::{NewEntryHeader, WireDelete},
    metadata::{EntryDhtStatus, MetadataSet, TimedHeaderHash},
    Entry, EntryHashed, HeaderHashed,
};
use holochain_zome_types::link::Link;
use std::collections::{BTreeSet, HashMap};
use tracing::*;

#[cfg(test)]
mod network_tests;
#[cfg(test)]
mod test;

pub mod error;

pub struct Cascade<'env: 'a, 'a, M = MetadataBuf<'env>, C = MetadataBuf<'env>>
where
    M: MetadataBufT,
    C: MetadataBufT,
{
    element_vault: &'a ChainCasBuf<'env>,
    meta_vault: &'a M,

    element_cache: &'a mut ChainCasBuf<'env>,
    meta_cache: &'a mut C,

    network: HolochainP2pCell,
}

/// The state of the cascade search
enum Search {
    /// The entry is found and we can stop
    Found(ChainElement),
    /// We haven't found the entry yet and should
    /// continue searching down the cascade
    Continue(HeaderHash),
    /// We haven't found the entry and should
    /// not continue searching down the cascade
    // TODO This information is currently not passed back to
    // the caller however it might be useful.
    NotInCascade,
}

/// Should these functions be sync or async?
/// Depends on how much computation, and if writes are involved
impl<'env: 'a, 'a, M, C> Cascade<'env, 'a, M, C>
where
    C: MetadataBufT,
    M: MetadataBufT,
{
    /// Constructs a [Cascade], taking references to all necessary databases
    pub fn new(
        element_vault: &'a ChainCasBuf<'env>,
        meta_vault: &'a M,
        element_cache: &'a mut ChainCasBuf<'env>,
        meta_cache: &'a mut C,
        network: HolochainP2pCell,
    ) -> Self {
        Cascade {
            element_vault,
            meta_vault,
            element_cache,
            meta_cache,
            network,
        }
    }

    async fn fetch_element_via_header(
        &mut self,
        hash: HeaderHash,
        options: GetOptions,
    ) -> CascadeResult<Option<ChainElement>> {
        let elements = self.network.get(hash.into(), options).await?;

        let mut element: Option<Box<WireElement>> = None;
        let proof_of_delete = elements.into_iter().find_map(|response| match response {
            // Has header
            GetElementResponse::GetHeader(Some(we)) => match we.deleted() {
                // Has proof of deleted entry
                Some(deleted) => Some((deleted.clone(), we)),
                // No proof of delete so this is a live element
                None => {
                    element = Some(we);
                    None
                }
            },
            // Doesn't have header but not because it was deleted
            GetElementResponse::GetHeader(None) => None,
            r @ _ => {
                error!(
                    msg = "Got an invalid response to fetch element via header",
                    ?r
                );
                None
            }
        });
        let ret = match (proof_of_delete, element) {
            // Found a delete.
            // Add it to the cache for future calls.
            (
                Some((
                    WireDelete {
                        element_delete_address,
                        removes_address,
                    },
                    element,
                )),
                _,
            ) => {
                let entry_hash = element
                    .entry_hash()
                    .cloned()
                    .expect("Deletes don't make sense on headers without entires");
                // TODO: Should / could we just do an integrate_to_cache here?
                // Add the header metadata
                let timed_header_hash: TimedHeaderHash = element
                    .into_element()
                    .await?
                    .into_inner()
                    .0
                    .into_header_and_signature()
                    .0
                    .into();
                self.meta_cache.register_raw_on_entry(
                    entry_hash.clone(),
                    SysMetaVal::NewEntry(timed_header_hash),
                )?;
                // Need to hash the entry here to add the delete
                self.meta_cache.register_raw_on_entry(
                    entry_hash,
                    SysMetaVal::Delete(element_delete_address.clone()),
                )?;
                self.meta_cache.register_raw_on_header(
                    removes_address,
                    SysMetaVal::Delete(element_delete_address),
                );
                None
            }
            // No deletes found, return the element if there was on
            (None, Some(element)) => {
                let element = element.into_element().await?;
                let (signed_header, maybe_entry) = element.clone().into_inner();

                // Hash entry
                let entry = match maybe_entry {
                    Some(entry) => Some(EntryHashed::from_content(entry).await),
                    None => None,
                };

                // Put in element element_cache
                self.element_cache.put(signed_header, entry)?;
                Some(element)
            }
            (None, None) => None,
        };
        Ok(ret)
    }

    async fn fetch_element_via_entry(
        &mut self,
        hash: EntryHash,
        options: GetOptions,
    ) -> CascadeResult<Option<(HashMap<HeaderHash, (NewEntryHeader, Signature)>, Entry)>> {
        let elements = self.network.get(hash.into(), options).await?;

        let mut ret_live_headers = HashMap::new();
        let mut ret_entry = None;
        debug!(num_ret_elements = elements.len());

        for element in elements {
            match element {
                GetElementResponse::GetEntryFull(Some(raw)) => {
                    let RawGetEntryResponse {
                        live_headers,
                        deletes,
                        entry,
                        entry_type,
                        entry_hash,
                    } = *raw;
                    if ret_entry.is_none() {
                        ret_entry = Some(entry);
                    }
                    for entry_header in live_headers {
                        let (new_entry_header, hash, signature) = entry_header
                            .create_new_entry_header(entry_type.clone(), entry_hash.clone())
                            .await;
                        ret_live_headers.insert(hash, (new_entry_header.clone(), signature));
                        self.meta_cache.register_header(new_entry_header).await?;
                    }
                    for WireDelete {
                        element_delete_address,
                        removes_address,
                    } in deletes
                    {
                        self.meta_cache.register_raw_on_header(
                            removes_address,
                            SysMetaVal::Delete(element_delete_address.clone()),
                        );
                        self.meta_cache.register_raw_on_entry(
                            entry_hash.clone(),
                            SysMetaVal::Delete(element_delete_address),
                        )?;
                    }
                }
                // Authority didn't have any headers for this entry
                GetElementResponse::GetEntryFull(None) => (),
                r @ GetElementResponse::GetHeader(_) => {
                    error!(
                        msg = "Got an invalid response to fetch element via entry",
                        ?r
                    );
                }
                r @ _ => unimplemented!("{:?} is unimplemented for fetching via entry", r),
            }
        }
        Ok(ret_entry.map(|e| (ret_live_headers, e)))
    }

    // TODO: Remove when used
    #[allow(dead_code)]
    async fn fetch_meta(
        &mut self,
        hash: AnyDhtHash,
        options: GetMetaOptions,
    ) -> CascadeResult<Vec<MetadataSet>> {
        let all_metadata = self.network.get_meta(hash.clone(), options).await?;

        // Only put raw meta data in element_cache and combine all results
        for metadata in all_metadata.iter().cloned() {
            let hash = hash.clone();
            // Put in meta element_cache
            let values = metadata
                .headers
                .into_iter()
                .map(|h| SysMetaVal::NewEntry(h))
                .chain(metadata.deletes.into_iter().map(|h| SysMetaVal::Delete(h)))
                .chain(metadata.updates.into_iter().map(|h| SysMetaVal::Update(h)));
            match *hash.hash_type() {
                hash_type::AnyDht::Entry(e) => {
                    let basis = hash.retype(e);
                    for v in values {
                        self.meta_cache.register_raw_on_entry(basis.clone(), v)?;
                    }
                }
                hash_type::AnyDht::Header => {
                    let basis = hash.retype(hash_type::Header);
                    for v in values {
                        self.meta_cache.register_raw_on_header(basis.clone(), v);
                    }
                }
            }
        }
        Ok(all_metadata)
    }

    /// Get a header without checking its metadata
    pub async fn dht_get_header_raw(
        &self,
        header_address: &HeaderHash,
    ) -> DatabaseResult<Option<SignedHeaderHashed>> {
        match self.element_vault.get_header(header_address).await? {
            None => self.element_cache.get_header(header_address).await,
            r => Ok(r),
        }
    }

    /// Get an entry without checking its metadata
    pub async fn dht_get_entry_raw(
        &self,
        entry_hash: &EntryHash,
    ) -> DatabaseResult<Option<EntryHashed>> {
        match self.element_vault.get_entry(entry_hash).await? {
            None => self.element_cache.get_entry(entry_hash).await,
            r => Ok(r),
        }
    }

    async fn get_element_local(&self, hash: &HeaderHash) -> CascadeResult<Option<ChainElement>> {
        match self.element_vault.get_element(hash).await? {
            None => Ok(self.element_cache.get_element(hash).await?),
            r => Ok(r),
        }
    }

    /// Returns the oldest live [ChainElement] for this [EntryHash] by getting the
    /// latest available metadata from authorities combined with this agents authored data.
    pub async fn dht_get_entry(
        &mut self,
        entry_hash: EntryHash,
    ) -> CascadeResult<Option<ChainElement>> {
        // Update the cache from the network
        let options = GetOptions {
            remote_agent_count: None,
            timeout_ms: None,
            as_race: false,
            race_timeout_ms: None,
            follow_redirects: false,
        };
        let result = self
            .fetch_element_via_entry(entry_hash.clone(), options.clone())
            .await?;

        // Meta Cache
        let oldest_live_element = match self.meta_cache.get_dht_status(&entry_hash)? {
            EntryDhtStatus::Live => {
                let oldest_live_header = self
                    .meta_cache
                    .get_headers(entry_hash)?
                    .min()?
                    .expect("Status is live but no headers?");

                match result {
                    Some((authority_headers, entry)) => {
                        match authority_headers.get(&oldest_live_header.header_hash) {
                            // Found the oldest header in the authorities data
                            Some((new_entry_header, signature)) => {
                                let header =
                                    HeaderHashed::from_content(new_entry_header.clone().into())
                                        .await;
                                Search::Found(ChainElement::new(
                                    SignedHeaderHashed::with_presigned(header, signature.clone()),
                                    Some(entry),
                                ))
                            }
                            // We have an oldest live header but it's not in the authorities data
                            None => self
                                .get_element_local(&oldest_live_header.header_hash)
                                .await?
                                .map(Search::Found)
                                .unwrap_or(Search::Continue(oldest_live_header.header_hash)),
                        }
                    }
                    // Not on network but we have live headers local
                    None => {
                        // Either we found it locally or we don't have it
                        self.get_element_local(&oldest_live_header.header_hash)
                            .await?
                            .map(Search::Found)
                            .unwrap_or(Search::NotInCascade)
                    }
                }
            }
            EntryDhtStatus::Dead
            | EntryDhtStatus::Pending
            | EntryDhtStatus::Rejected
            | EntryDhtStatus::Abandoned
            | EntryDhtStatus::Conflict
            | EntryDhtStatus::Withdrawn
            | EntryDhtStatus::Purged => Search::NotInCascade,
        };

        // Network
        match oldest_live_element {
            Search::Found(element) => Ok(Some(element)),
            Search::Continue(oldest_live_header) => {
                self.fetch_element_via_header(oldest_live_header, options)
                    .await
            }
            Search::NotInCascade => Ok(None),
        }
    }

    /// Returns the [ChainElement] for this [HeaderHash] if it is live
    /// by getting the latest available metadata from authorities
    /// combined with this agents authored data.
    /// _Note: Deleted headers are a tombstone set_
    pub async fn dht_get_header(
        &mut self,
        header_hash: HeaderHash,
    ) -> CascadeResult<Option<ChainElement>> {
        // Meta Cache
        if let Some(_) = self
            .meta_cache
            .get_deletes_on_header(header_hash.clone())?
            .next()?
        {
            // Final tombstone found
            return Ok(None);
        // Meta Vault
        } else if let Some(_) = self
            .meta_vault
            .get_deletes_on_header(header_hash.clone())?
            .next()?
        {
            // Final tombstone found
            return Ok(None);
        }
        // Network
        self.fetch_element_via_header(header_hash, GetOptions::default())
            .await
    }

    #[instrument(skip(self))]
    // Updates the cache with the latest network authority data
    // and returns what is in the cache.
    // This gives you the latest possible picture of the current dht state.
    // Data from your zome call is also added to the cache.
    pub async fn dht_get(&mut self, hash: AnyDhtHash) -> CascadeResult<Option<ChainElement>> {
        match *hash.hash_type() {
            AnyDht::Entry(e) => {
                let hash = hash.retype(e);
                self.dht_get_entry(hash).await
            }
            AnyDht::Header => {
                let hash = hash.retype(hash_type::Header);
                self.dht_get_header(hash).await
            }
        }
    }

    /// Gets an links from the cas or cache depending on it's metadata
    // The default behavior is to skip deleted or replaced entries.
    // TODO: Implement customization of this behavior with an options/builder struct
    pub async fn dht_get_links<'link>(
        &self,
        key: &'link LinkMetaKey<'link>,
    ) -> DatabaseResult<Vec<Link>> {
        // Meta Cache
        // Return any links from the meta cache that don't have removes.
        self.meta_cache
            .get_links(key)?
            .map(|l| Ok(l.into_link()))
            .collect()
    }
}

#[cfg(test)]
/// Helper function for easily setting up cascades during tests
pub fn test_dbs_and_mocks<'env>(
    reader: &'env holochain_state::transaction::Reader<'env>,
    dbs: &impl holochain_state::db::GetDb,
) -> (
    ChainCasBuf<'env>,
    super::metadata::MockMetadataBuf,
    ChainCasBuf<'env>,
    super::metadata::MockMetadataBuf,
) {
    let cas = ChainCasBuf::vault(&reader, dbs, true).unwrap();
    let element_cache = ChainCasBuf::cache(&reader, dbs).unwrap();
    let metadata = super::metadata::MockMetadataBuf::new();
    let metadata_cache = super::metadata::MockMetadataBuf::new();
    (cas, metadata, element_cache, metadata_cache)
}
