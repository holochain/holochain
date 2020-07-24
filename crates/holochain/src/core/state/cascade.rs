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
    metadata::{EntryDhtStatus, LinkMetaKey, MetadataBuf, MetadataBufT},
};
use error::CascadeResult;
use fallible_iterator::FallibleIterator;
use holo_hash_core::{
    hash_type::{self, AnyDht},
    AnyDhtHash, EntryHash, HeaderAddress, HeaderHash,
};
use holochain_p2p::{actor::GetOptions, HolochainP2pCell};
use holochain_state::error::DatabaseResult;
use holochain_types::{
    element::{ChainElement, SignedHeaderHashed},
    EntryHashed,
};
use holochain_zome_types::link::Link;
use tracing::*;

#[cfg(test)]
mod network_tests;
#[cfg(test)]
mod test;

mod error;

pub struct Cascade<'env: 'a, 'a, M = MetadataBuf<'env>, C = MetadataBuf<'env>>
where
    M: MetadataBufT,
    C: MetadataBufT,
{
    primary: &'a ChainCasBuf<'env>,
    primary_meta: &'a M,

    cache: &'a mut ChainCasBuf<'env>,
    cache_meta: &'a C,

    network: HolochainP2pCell,
}

/// The state of the cascade search
enum Search {
    /// The entry is found and we can stop
    Found(ChainElement),
    /// We haven't found the entry yet and should
    /// continue searching down the cascade
    Continue,
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
    /// Constructs a [Cascade], taking references to a CAS and a cache
    pub fn new(
        primary: &'a ChainCasBuf<'env>,
        primary_meta: &'a M,
        cache: &'a mut ChainCasBuf<'env>,
        cache_meta: &'a C,
        network: HolochainP2pCell,
    ) -> Self {
        Cascade {
            primary,
            primary_meta,
            cache,
            cache_meta,
            network,
        }
    }

    // TODO: Remove when used
    #[allow(dead_code)]
    async fn fetch_element(
        &mut self,
        hash: AnyDhtHash,
        options: GetOptions,
    ) -> CascadeResult<Option<ChainElement>> {
        let elements = self.network.get(hash, options).await?;

        // TODO: handle case of multiple elements returned
        // Get the first returned element
        let element = match elements.into_iter().next() {
            Some(chain_element_data) => {
                // Deserialize to type and hash
                let element = chain_element_data.into_element().await?;
                let (signed_header, maybe_entry) = element.clone().into_inner();

                // Hash entry
                let entry = match maybe_entry {
                    Some(entry) => Some(EntryHashed::with_data(entry).await?),
                    None => None,
                };

                // Put in element cache
                self.cache.put(signed_header, entry)?;
                Some(element)
            }
            None => None,
        };
        Ok(element)
    }

    /// Get a header without checking its metadata
    pub async fn dht_get_header_raw(
        &self,
        header_address: &HeaderAddress,
    ) -> DatabaseResult<Option<SignedHeaderHashed>> {
        match self.primary.get_header(header_address).await? {
            None => self.cache.get_header(header_address).await,
            r => Ok(r),
        }
    }

    /// Get an entry without checking its metadata
    pub async fn dht_get_entry_raw(
        &self,
        entry_hash: &EntryHash,
    ) -> DatabaseResult<Option<EntryHashed>> {
        match self.primary.get_entry(entry_hash).await? {
            None => self.cache.get_entry(entry_hash).await,
            r => Ok(r),
        }
    }

    /// Returns the oldest live [ChainElement] for this [EntryHash] by getting the
    /// latest available metadata from authorities combined with this agents authored data.
    pub async fn dht_get_entry(
        &self,
        entry_hash: EntryHash,
    ) -> DatabaseResult<Option<ChainElement>> {
        // TODO: Update the cache from the network
        // TODO: Fetch the EntryDhtStatus
        // TODO: Fetch the headers on this entry
        // TODO: Fetch the deletes on this entry
        // TODO: Update the meta cache

        // Meta Cache
        let oldest_live_element = match self.cache_meta.get_dht_status(&entry_hash)? {
            EntryDhtStatus::Live => {
                // TODO: PERF: Firstly probably do this on writes not reads to meta cache
                // Secondly figure out how to allow these iterators to cross awaits to avoid collecting
                let headers = self
                    .cache_meta
                    .get_headers(entry_hash)?
                    .collect::<Vec<_>>()?;
                let mut oldest = None;
                let mut result = None;
                for hash in headers {
                    // Element Vault
                    // TODO: Handle error
                    let element = match self.primary.get_element(&hash).await.unwrap() {
                        // Element Cache
                        // TODO: Handle error
                        None => self.cache.get_element(&hash).await.unwrap(),
                        e => e,
                    };
                    if let Some(element) = element {
                        let t = element.header().timestamp();
                        let o = oldest.get_or_insert(t);
                        if t < *o {
                            *o = t;
                            result = Some(element);
                        }
                    }
                }
                result.map(Search::Found).unwrap_or(Search::Continue)
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
            Search::Continue => {
                // TODO: Fetch the element from the network
                // TODO: Update the element cache
                // TODO: Return the element
                todo!("Fetch from network")
            }
            Search::NotInCascade => Ok(None),
        }
    }

    /// Returns the [ChainElement] for this [HeaderHash] if it is live
    /// by getting the latest available metadata from authorities
    /// combined with this agents authored data.
    /// _Note: Deleted headers are a tombstone set_
    pub async fn dht_get_header(
        &self,
        header_hash: HeaderHash,
    ) -> DatabaseResult<Option<ChainElement>> {
        // Meta Cache
        if let Some(_) = self
            .cache_meta
            .get_deletes_on_header(header_hash.clone())?
            .next()?
        {
            // Final tombstone found
            return Ok(None);
        // Meta Vault
        } else if let Some(_) = self
            .primary_meta
            .get_deletes_on_header(header_hash.clone())?
            .next()?
        {
            // Final tombstone found
            return Ok(None);
        }
        // TODO: Network
        // TODO: Fetches any deletes on this header.
        // TODO: If found updates the meta cache and returns none.

        // Element Vault
        // Checks the element vault for this header element.
        // TODO: Handle error
        let element = match self.primary.get_element(&header_hash).await.unwrap() {
            // Element Cache
            // If not found checks the element cache
            // TODO: Handle error
            None => self.cache.get_element(&header_hash).await.unwrap(),
            e => e,
        };

        // Network
        match element {
            None => {
                // TODO: If not found fetches this element from the network.
                // TODO: Update the element cache.
                todo!("Fetch element from the network")
            }
            e => Ok(e),
        }
    }

    #[instrument(skip(self))]
    // Updates the cache with the latest network authority data
    // and returns what is in the cache.
    // This gives you the latest possible picture of the current dht state.
    // Data from your zome call is also added to the cache.
    pub async fn dht_get(&self, hash: AnyDhtHash) -> DatabaseResult<Option<ChainElement>> {
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
        self.cache_meta
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
    let cas = ChainCasBuf::primary(&reader, dbs, true).unwrap();
    let cache = ChainCasBuf::cache(&reader, dbs).unwrap();
    let metadata = super::metadata::MockMetadataBuf::new();
    let metadata_cache = super::metadata::MockMetadataBuf::new();
    (cas, metadata, cache, metadata_cache)
}
