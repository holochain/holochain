//! # Cascade
//! ## Dimensions
//! get vs get_links
//! default vs options
//! fast vs strict (is set by app dev)
//!
//! ## Get
//! ### Default - Get's the latest version
//! Scratch Live -> Return
//! Scratch NotFound -> Goto Cas
//! Scratch _ -> None
//! Cas Live -> Return
//! Cas NotFound -> Goto cache
//! Cas _ -> None
//! Cache Live -> Return
//! Cache Pending -> Goto Network
//! Cache NotFound -> Goto Network
//! Cache _ -> None
//!
//! ## Get Links
//! ### Default - Get's the latest version
//! if I'm an authority
//! Scratch Found-> Return
//! Scratch NotFound -> Goto Cas
//! Cas Found -> Return
//! Cas NotFound -> Goto Network
//! else
//! Network Found -> Return
//! Network NotFound -> Goto Cache
//! Cache Found -> Return
//! Cache NotFound -> None
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
use std::collections::HashSet;
use sx_state::{error::DatabaseResult, prelude::Reader};
use sx_types::{entry::Entry, persistence::cas::content::Address};
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

enum Search {
    Found(Entry),
    Continue,
    NotFound,
}

/// Should these functions be sync or async?
/// Depends on how much computation, and if writes are involved
impl<'env, C> Cascade<'env, C>
where
    C: ChainMetaBufT<'env>,
{
    /// Take references to cas and cache
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
    pub async fn dht_get(&self, address: Address) -> DatabaseResult<Option<Entry>> {
        // Cas
        let search = self
            .primary
            .get_entry(&address)?
            .and_then(|entry| {
                self.primary_meta.get_crud(&address).ok().map(|crud| {
                    if let EntryDhtStatus::Live = crud {
                        Search::Found(entry)
                    } else {
                        Search::NotFound
                    }
                })
            })
            .unwrap_or_else(|| Search::Continue);

        // Cache
        match search {
            Search::Continue => self
                .cache
                .get_entry(&address)?
                .and_then(|entry| {
                    self.cache_meta
                        .get_crud(&address)
                        .ok()
                        .and_then(|crud| match crud {
                            EntryDhtStatus::Live => Some(entry),
                            _ => None,
                        })
                })
                .map(Ok)
                .transpose(),
            Search::Found(entry) => Ok(Some(entry)),
            Search::NotFound => Ok(None),
        }
    }

    pub async fn dht_get_links<S: Into<String>>(
        &self,
        base: Address,
        tag: S,
    ) -> DatabaseResult<HashSet<Address>> {
        // Am I an authority?
        let authority = self.primary.get_entry(&base)?.is_some();
        let tag = tag.into();
        if authority {
            // Cas
            let links = self.primary_meta.get_links(&base, tag.clone())?;

            // Cache
            if links.is_empty() {
                self.cache_meta.get_links(&base, tag)
            } else {
                Ok(links)
            }
        } else {
            // Cache
            self.cache_meta.get_links(&base, tag)
        }
    }
}
