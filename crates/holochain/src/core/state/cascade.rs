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
    element_buf::ElementBuf,
    metadata::{LinkMetaKey, LinkMetaVal, MetadataBuf, MetadataBufT, SysMetaVal},
};
use error::CascadeResult;
use holo_hash::{hash_type, AnyDhtHash, EntryHash, HeaderHash};
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
    element_vault: &'a ElementBuf<'env>,
    meta_vault: &'a M,

    element_cache: &'a mut ElementBuf<'env>,
    meta_cache: &'a mut C,

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
    /// Constructs a [Cascade], taking references to all necessary databases
    pub fn new(
        element_vault: &'a ElementBuf<'env>,
        meta_vault: &'a M,
        element_cache: &'a mut ElementBuf<'env>,
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
                    Some(entry) => Some(EntryHashed::from_content(entry).await),
                    None => None,
                };

                // Put in element element_cache
                self.element_cache.put(signed_header, entry)?;
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

    // TODO: dht_get_header -> Header

    #[instrument(skip(self))]
    /// Gets an entry from the vault or cache depending on its metadata
    // TODO asyncify slow blocking functions here
    // The default behavior is to skip deleted or replaced entries.
    // TODO: Implement customization of this behavior with an options/builder struct
    pub async fn dht_get(&self, entry_hash: &EntryHash) -> DatabaseResult<Option<EntryHashed>> {
        // Cas
        let search = self
            .element_vault
            .get_entry(entry_hash)
            .await?
            .and_then(|entry| {
                self.meta_vault.get_dht_status(entry_hash).ok().map(|crud| {
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
            Search::Continue => {
                Ok(self
                    .element_cache
                    .get_entry(entry_hash)
                    .await?
                    .and_then(|entry| {
                        self.meta_cache.get_dht_status(entry_hash).ok().and_then(
                            |crud| match crud {
                                EntryDhtStatus::Live => Some(entry),
                                _ => None,
                            },
                        )
                    }))
            }
            Search::Found(entry) => Ok(Some(entry)),
            Search::NotInCascade => Ok(None),
        }
    }

    /// Gets links from the vault or cache depending on its metadata
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
        let authority = self.element_vault.contains(&key.base()).await?;
        if authority {
            // Cas
            let links = self.meta_vault.get_links(key)?;

            // TODO: Why check element_cache if you are the authority?
            // Cache
            if links.is_empty() {
                self.meta_cache.get_links(key)
            } else {
                Ok(links)
            }
        } else {
            // TODO: Why check element_cache if you need to go to the authority?
            // Cache
            self.meta_cache.get_links(key)
        }
    }
}

#[cfg(test)]
/// Helper function for easily setting up cascades during tests
pub fn test_dbs_and_mocks<'env>(
    reader: &'env holochain_state::transaction::Reader<'env>,
    dbs: &impl holochain_state::db::GetDb,
) -> (
    ElementBuf<'env>,
    super::metadata::MockMetadataBuf,
    ElementBuf<'env>,
    super::metadata::MockMetadataBuf,
) {
    let cas = ElementBuf::vault(&reader, dbs, true).unwrap();
    let element_cache = ElementBuf::cache(&reader, dbs).unwrap();
    let metadata = super::metadata::MockMetadataBuf::new();
    let metadata_cache = super::metadata::MockMetadataBuf::new();
    (cas, metadata, element_cache, metadata_cache)
}
