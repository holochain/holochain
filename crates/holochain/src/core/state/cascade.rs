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
    chain_meta::{ChainMetaBufT, EntryDhtStatus},
};
use holo_hash::EntryHash;
use std::collections::HashSet;
use sx_state::{error::DatabaseResult, prelude::Reader};
use sx_types::entry::Entry;
use tracing::*;

#[cfg(test)]
mod test;

pub struct Cascade<'env, C>
where
    C: ChainMetaBufT<'env>,
{
    primary: &'env ChainCasBuf<'env, Reader<'env>>,
    primary_meta: &'env C,

    cache: &'env ChainCasBuf<'env, Reader<'env>>,
    cache_meta: &'env C,
}

/// The state of the cascade search
enum Search {
    /// The entry is found and we can stop
    Found(Entry),
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
impl<'env, C> Cascade<'env, C>
where
    C: ChainMetaBufT<'env>,
{
    /// Constructs a [Cascade], taking references to a CAS and a cache
    pub fn new(
        primary: &'env ChainCasBuf<'env, Reader<'env>>,
        primary_meta: &'env C,
        cache: &'env ChainCasBuf<'env, Reader<'env>>,
        cache_meta: &'env C,
    ) -> Self {
        Cascade {
            primary,
            primary_meta,
            cache,
            cache_meta,
        }
    }

    #[instrument(skip(self))]
    /// Gets an entry from the cas or cache depending on it's metadata
    // TODO asyncify slow blocking functions here
    // The default behavior is to skip deleted or replaced entries.
    // TODO: Implement customization of this behavior with an options/builder struct
    pub async fn dht_get(&self, entry_hash: EntryHash) -> DatabaseResult<Option<Entry>> {
        // Cas
        let search = self
            .primary
            .get_entry(entry_hash.clone())?
            .and_then(|entry| {
                self.primary_meta
                    .get_crud(entry_hash.clone())
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
            Search::Continue => Ok(self.cache.get_entry(entry_hash.clone())?.and_then(|entry| {
                self.cache_meta
                    .get_crud(entry_hash)
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
    pub async fn dht_get_links<S: Into<String>>(
        &self,
        base: EntryHash,
        tag: S,
    ) -> DatabaseResult<HashSet<EntryHash>> {
        // Am I an authority?
        let authority = self.primary.contains(base.clone())?;
        let tag = tag.into();
        if authority {
            // Cas
            let links = self.primary_meta.get_links(base.clone(), tag.clone())?;

            // Cache
            if links.is_empty() {
                self.cache_meta.get_links(base, tag)
            } else {
                Ok(links)
            }
        } else {
            // Cache
            self.cache_meta.get_links(base, tag)
        }
    }
}
