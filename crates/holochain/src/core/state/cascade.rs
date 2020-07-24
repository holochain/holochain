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
    metadata::{LinkMetaKey, LinkMetaVal, MetadataBuf, MetadataBufT, SysMetaVal},
};
use error::CascadeResult;
use holo_hash_core::{hash_type, AnyDhtHash, EntryHash, HeaderAddress};
use holochain_p2p::{
    actor::{GetMetaOptions, GetOptions},
    HolochainP2pCell,
};
use holochain_state::error::DatabaseResult;
use holochain_types::{
    element::{ChainElement, SignedHeaderHashed},
    metadata::{EntryDhtStatus, MetadataSet},
    EntryHashed,
};
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
    cache_meta: &'a mut C,

    network: HolochainP2pCell,
}

/// The state of the cascade search
enum Search {
    /// The entry is found and we can stop
    Found(EntryHashed),
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
        cache_meta: &'a mut C,
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

    // TODO: Remove when used
    #[allow(dead_code)]
    async fn fetch_meta(
        &mut self,
        hash: AnyDhtHash,
        options: GetMetaOptions,
    ) -> CascadeResult<Vec<MetadataSet>> {
        let all_metadata = self.network.get_meta(hash.clone(), options).await?;

        // Only put raw meta data in cache and combine all results
        for metadata in all_metadata.iter().cloned() {
            let hash = hash.clone();
            // Put in meta cache
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
                        self.cache_meta.register_raw_on_entry(basis.clone(), v)?;
                    }
                }
                hash_type::AnyDht::Header => {
                    let basis = hash.retype(hash_type::Header);
                    for v in values {
                        self.cache_meta.register_raw_on_header(basis.clone(), v);
                    }
                }
            }
        }
        Ok(all_metadata)
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

    // TODO: dht_get_header -> Header

    #[instrument(skip(self))]
    /// Gets an entry from the cas or cache depending on it's metadata
    // TODO asyncify slow blocking functions here
    // The default behavior is to skip deleted or replaced entries.
    // TODO: Implement customization of this behavior with an options/builder struct
    pub async fn dht_get(&self, entry_hash: &EntryHash) -> DatabaseResult<Option<EntryHashed>> {
        // Cas
        let search = self
            .primary
            .get_entry(entry_hash)
            .await?
            .and_then(|entry| {
                self.primary_meta
                    .get_dht_status(entry_hash)
                    .ok()
                    .map(|crud| {
                        if let EntryDhtStatus::Live = crud {
                            Search::Found(entry)
                        } else {
                            Search::NotInCascade
                        }
                    })
            })
            .unwrap_or_else(|| Search::Continue);

        // Cache
        match search {
            Search::Continue => Ok(self.cache.get_entry(entry_hash).await?.and_then(|entry| {
                self.cache_meta
                    .get_dht_status(entry_hash)
                    .ok()
                    .and_then(|crud| match crud {
                        EntryDhtStatus::Live => Some(entry),
                        _ => None,
                    })
            })),
            Search::Found(entry) => Ok(Some(entry)),
            Search::NotInCascade => Ok(None),
        }
    }

    /// Gets an links from the cas or cache depending on it's metadata
    // TODO asyncify slow blocking functions here
    // The default behavior is to skip deleted or replaced entries.
    // TODO: Implement customization of this behavior with an options/builder struct
    pub async fn dht_get_links<'link>(
        &self,
        key: &'link LinkMetaKey<'link>,
    ) -> DatabaseResult<Vec<LinkMetaVal>> {
        // Am I an authority?
        // TODO: Not a good check for authority as the base could be in the cas because
        // you authored it.
        let authority = self.primary.contains(&key.base()).await?;
        if authority {
            // Cas
            let links = self.primary_meta.get_links(key)?;

            // TODO: Why check cache if you are the authority?
            // Cache
            if links.is_empty() {
                self.cache_meta.get_links(key)
            } else {
                Ok(links)
            }
        } else {
            // TODO: Why check cache if you need to go to the authority?
            // Cache
            self.cache_meta.get_links(key)
        }
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
