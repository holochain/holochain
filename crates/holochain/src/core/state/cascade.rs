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
use crate::core::workflow::integrate_dht_ops_workflow::integrate_single_metadata;
use error::CascadeResult;
use fallible_iterator::FallibleIterator;
use holo_hash::{
    hash_type::{self, AnyDht},
    AnyDhtHash, EntryHash, HeaderHash,
};
use holochain_p2p::{
    actor::{GetLinksOptions, GetMetaOptions, GetOptions},
    HolochainP2pCell,
};
use holochain_state::error::DatabaseResult;
use holochain_types::{
    dht_op::{produce_op_lights_from_element_group, produce_op_lights_from_elements},
    element::{
        Element, ElementGroup, GetElementResponse, RawGetEntryResponse, SignedHeaderHashed,
        SignedHeaderHashedExt,
    },
    entry::option_entry_hashed,
    link::{GetLinksResponse, WireLinkMetaKey},
    metadata::{EntryDhtStatus, MetadataSet},
    EntryHashed,
};
use holochain_zome_types::{element::SignedHeader, link::Link};
use tracing::*;

#[cfg(test)]
mod network_tests;
#[cfg(all(test, outdated_tests))]
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

#[derive(Debug)]
/// The state of the cascade search
enum Search {
    /// The entry is found and we can stop
    Found(Element),
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

    async fn update_stores(&mut self, element: Element) -> CascadeResult<()> {
        let op_lights = produce_op_lights_from_elements(vec![&element]).await?;
        let (shh, e) = element.into_inner();
        self.element_cache.put(shh, option_entry_hashed(e).await)?;
        for op in op_lights {
            integrate_single_metadata(op, &self.element_cache, self.meta_cache).await?
        }
        Ok(())
    }

    async fn update_stores_with_element_group(
        &mut self,
        elements: ElementGroup<'_>,
    ) -> CascadeResult<()> {
        let op_lights = produce_op_lights_from_element_group(&elements).await?;
        self.element_cache.put_element_group(elements)?;
        for op in op_lights {
            integrate_single_metadata(op, &self.element_cache, self.meta_cache).await?
        }
        Ok(())
    }

    async fn fetch_element_via_header(
        &mut self,
        hash: HeaderHash,
        options: GetOptions,
    ) -> CascadeResult<()> {
        let results = self.network.get(hash.into(), options).await?;
        // Search through the returns for the first delete
        for response in results.into_iter() {
            match response {
                // Has header
                GetElementResponse::GetHeader(Some(we)) => {
                    let (element, delete) = we.into_element_and_delete().await;
                    self.update_stores(element).await?;

                    if let Some(delete) = delete {
                        self.update_stores(delete).await?;
                    }
                }
                // Doesn't have header but not because it was deleted
                GetElementResponse::GetHeader(None) => (),
                r @ _ => {
                    error!(
                        msg = "Got an invalid response to fetch element via header",
                        ?r
                    );
                }
            }
        }
        Ok(())
    }

    #[instrument(skip(self, options))]
    async fn fetch_element_via_entry(
        &mut self,
        hash: EntryHash,
        options: GetOptions,
    ) -> CascadeResult<()> {
        let results = self.network.get(hash.clone().into(), options).await?;

        for response in results {
            match response {
                GetElementResponse::GetEntryFull(Some(raw)) => {
                    let RawGetEntryResponse {
                        live_headers,
                        deletes,
                        entry,
                        entry_type,
                    } = *raw;
                    let elements =
                        ElementGroup::from_wire_elements(live_headers, entry_type, entry).await?;
                    self.update_stores_with_element_group(elements).await?;
                    for delete in deletes {
                        let element = delete.into_element().await;
                        self.update_stores(element).await?;
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
        Ok(())
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

    async fn fetch_links(
        &mut self,
        link_key: WireLinkMetaKey,
        options: GetLinksOptions,
    ) -> CascadeResult<()> {
        let results = self.network.get_links(link_key, options).await?;
        for links in results {
            let GetLinksResponse {
                link_adds,
                link_removes,
            } = links;

            for (link_add, signature) in link_adds {
                let element = Element::new(
                    SignedHeaderHashed::from_content(SignedHeader(link_add.into(), signature))
                        .await,
                    None,
                );
                self.update_stores(element).await?;
            }
            for (link_remove, signature) in link_removes {
                let element = Element::new(
                    SignedHeaderHashed::from_content(SignedHeader(link_remove.into(), signature))
                        .await,
                    None,
                );
                self.update_stores(element).await?;
            }
        }
        Ok(())
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

    async fn get_element_local_raw(&self, hash: &HeaderHash) -> CascadeResult<Option<Element>> {
        match self.element_vault.get_element(hash).await? {
            None => Ok(self.element_cache.get_element(hash).await?),
            r => Ok(r),
        }
    }

    /// Returns the oldest live [Element] for this [EntryHash] by getting the
    /// latest available metadata from authorities combined with this agents authored data.
    pub async fn dht_get_entry(
        &mut self,
        entry_hash: EntryHash,
        options: GetOptions,
    ) -> CascadeResult<Option<Element>> {
        // Update the cache from the network
        self.fetch_element_via_entry(entry_hash.clone(), options.clone())
            .await?;

        // Meta Cache
        let oldest_live_element = match self.meta_cache.get_dht_status(&entry_hash)? {
            EntryDhtStatus::Live => {
                let oldest_live_header = self
                    .meta_cache
                    .get_headers(entry_hash)?
                    .filter_map(|header| {
                        if let None = self
                            .meta_cache
                            .get_deletes_on_header(header.header_hash.clone())?
                            .next()?
                        {
                            Ok(Some(header))
                        } else {
                            Ok(None)
                        }
                    })
                    .min()?
                    .expect("Status is live but no headers?");

                // We have an oldest live header now get the element
                self.get_element_local_raw(&oldest_live_header.header_hash)
                    .await?
                    .map(Search::Found)
                    // It's not local so check the network
                    .unwrap_or(Search::Continue(oldest_live_header.header_hash))
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
                self.dht_get_header(oldest_live_header, options).await
            }
            Search::NotInCascade => Ok(None),
        }
    }

    /// Returns the [Element] for this [HeaderHash] if it is live
    /// by getting the latest available metadata from authorities
    /// combined with this agents authored data.
    /// _Note: Deleted headers are a tombstone set_
    pub async fn dht_get_header(
        &mut self,
        header_hash: HeaderHash,
        options: GetOptions,
    ) -> CascadeResult<Option<Element>> {
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
        self.fetch_element_via_header(header_hash.clone(), options)
            .await?;
        // Check if header is alive after fetch
        let delete_on_header = self
            .meta_cache
            .get_deletes_on_header(header_hash.clone())?
            .next()?
            .is_none();

        if delete_on_header {
            Ok(None)
        } else {
            // See if the header was found?
            self.get_element_local_raw(&header_hash).await
        }
    }

    #[instrument(skip(self))]
    // Updates the cache with the latest network authority data
    // and returns what is in the cache.
    // This gives you the latest possible picture of the current dht state.
    // Data from your zome call is also added to the cache.
    pub async fn dht_get(
        &mut self,
        hash: AnyDhtHash,
        options: GetOptions,
    ) -> CascadeResult<Option<Element>> {
        match *hash.hash_type() {
            AnyDht::Entry(e) => {
                let hash = hash.retype(e);
                self.dht_get_entry(hash, options).await
            }
            AnyDht::Header => {
                let hash = hash.retype(hash_type::Header);
                self.dht_get_header(hash, options).await
            }
        }
    }

    /// Gets an links from the cas or cache depending on it's metadata
    // The default behavior is to skip deleted or replaced entries.
    // TODO: Implement customization of this behavior with an options/builder struct
    pub async fn dht_get_links<'link>(
        &mut self,
        key: &'link LinkMetaKey<'link>,
        options: GetLinksOptions,
    ) -> CascadeResult<Vec<Link>> {
        // Update the cache from the network
        self.fetch_links(key.into(), options).await?;

        // Meta Cache
        // Return any links from the meta cache that don't have removes.
        Ok(self
            .meta_cache
            .get_links(key)?
            .map(|l| Ok(l.into_link()))
            .collect()?)
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
