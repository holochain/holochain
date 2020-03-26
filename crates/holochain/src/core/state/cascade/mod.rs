//! get vs get_links
//! default vs options
//! fast vs strict #set by app dev
//!
//! get Default - Get's the latest version
//! Scratch if Live -> Return
//! Scratch if Dead -> None
//! Scratch NotFound -> Goto Cas
//! Cas Live -> Return
//! Cas NotFound -> Goto cache
//! Cas _ -> None
//! Cache Live -> Return
//! Cache Pending -> Goto Network
//! Cache NotFound -> Goto Network
//! Cache _ -> None
//!
//! get_links Default - Get's the latest version
//! Always try authority
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
//! gets most recent N links with default N (50)
//! Page number
//! load_true loads the results into cache

use super::{
    chain_cas::ChainCasBuf,
    chain_meta::{ChainMetaBufT, Crud},
};
use crate::core::net::NetRequester;
use std::collections::HashSet;
use sx_state::{
    error::{DatabaseError, DatabaseResult},
    prelude::Reader,
};
use sx_types::{entry::Entry, persistence::cas::content::Address};
use tracing::*;

#[cfg(test)]
mod test;

/// TODO Network is not handled here, must either return
/// the fact that a network get is required or take a reference to the network.
#[allow(dead_code)]
pub struct Cascade<'env, C, N>
where
    C: ChainMetaBufT<'env>,
    N: NetRequester,
{
    primary: &'env ChainCasBuf<'env, Reader<'env>>,
    primary_meta: &'env C,

    cache: &'env ChainCasBuf<'env, Reader<'env>>,
    cache_meta: &'env C,

    network: N,
}

enum Search {
    Found(Entry),
    Continue,
    NotFound,
}

/// Should these functions be sync or async?
/// Depends on how much computation, and if writes are involved
impl<'env, C, N> Cascade<'env, C, N>
where
    C: ChainMetaBufT<'env>,
    N: NetRequester,
{
    /// Take references to cas and cache
    pub fn new(
        primary: &'env ChainCasBuf<'env, Reader<'env>>,
        primary_meta: &'env C,
        cache: &'env ChainCasBuf<'env, Reader<'env>>,
        cache_meta: &'env C,
        network: N,
    ) -> Self {
        Cascade {
            primary,
            primary_meta,
            cache,
            cache_meta,
            network,
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
                    if let Crud::Live = crud {
                        Search::Found(entry)
                    } else {
                        Search::NotFound
                    }
                })
            })
            .unwrap_or_else(|| Search::Continue);

        // Cache
        let search = match search {
            Search::Continue => self
                .cache
                .get_entry(&address)?
                .and_then(|entry| {
                    self.cache_meta
                        .get_crud(&address)
                        .ok()
                        .map(|crud| match crud {
                            Crud::Live => Search::Found(entry),
                            Crud::Pending => Search::Continue,
                            _ => Search::NotFound,
                        })
                })
                .unwrap_or_else(|| Search::Continue),
            Search::Found(entry) => return Ok(Some(entry)),
            Search::NotFound => return Ok(None),
        };

        // Network
        match search {
            Search::Continue => self
                .network
                .fetch_entry(&address)
                .map_err(|e| DatabaseError::Other(e.into())),
            Search::Found(entry) => return Ok(Some(entry)),
            Search::NotFound => return Ok(None),
        }
    }
    pub async fn dht_get_links<S: Into<String>>(
        &self,
        base: Address,
        tag: S,
    ) -> DatabaseResult<HashSet<Address>> {
        let tag = tag.into();
        // Cas
        let links = self.primary_meta.get_links(&base, tag.clone())?;

        // Cache
        let links = if links.len() == 0 {
            self.cache_meta.get_links(&base, tag.clone())?
        } else {
            links
        };
        // Network
        if links.len() == 0 {
            self.network
                .fetch_links(&base, tag)
                .map_err(|e| DatabaseError::Other(e.into()))
        } else {
            Ok(links)
        }
    }
}
