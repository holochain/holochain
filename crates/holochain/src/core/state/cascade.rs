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
    metadata::{EntryDhtStatus, LinkMetaKey, LinkMetaVal, MetadataBuf, MetadataBufT},
};
use holo_hash::Hashable;
use holochain_p2p::{actor::GetOptions, HolochainP2pCell};
use holochain_serialized_bytes::prelude::*;
use holochain_state::error::DatabaseResult;
use holochain_types::{
    composite_hash::{AnyDhtHash, EntryHash, HeaderAddress},
    element::{ChainElement, SignedHeader, SignedHeaderHashed},
    Entry, EntryHashed, HeaderHashed,
};
use tracing::*;

#[cfg(test)]
mod network_tests;
#[cfg(test)]
mod test;

// TODO: Remove this when holohash refactor PR lands
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, SerializedBytes)]
struct PlaceholderGetReturn {
    signed_header: SignedHeader,
    entry: Option<Entry>,
}

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

    async fn fetch_element(
        &mut self,
        hash: AnyDhtHash,
        options: GetOptions,
    ) -> DatabaseResult<Option<ChainElement>> {
        // TODO: Handle error
        let elements = self.network.get(hash, options).await.unwrap();
        // TODO: handle case of multiple elements returned
        let element = match elements.into_iter().next() {
            Some(bytes) => {
                // TODO: Handle error
                let element = PlaceholderGetReturn::try_from(bytes).unwrap();
                let (header, signature) = element.signed_header.into();
                // TODO: Handle error
                let header = HeaderHashed::with_data(header).await?;

                // TODO: Does this verify the signature?
                let signed_header = SignedHeaderHashed::with_presigned(header, signature);
                let element = ChainElement::new(signed_header, element.entry);
                let (signed_header, maybe_entry) = element.clone().into_inner();
                let entry = match maybe_entry {
                    Some(entry) => Some(EntryHashed::with_data(entry).await?),
                    None => None,
                };
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
