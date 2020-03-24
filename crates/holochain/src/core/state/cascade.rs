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

use super::{chain_cas::ChainCasBuf, chain_meta::ChainMetaBuf};
use sx_state::prelude::Reader;
use sx_types::{entry::Entry, persistence::cas::content::Address};

/// TODO Network is not handled here, must either return
/// the fact that a network get is required or take a reference to the network.
#[allow(dead_code)]
pub struct Cascade<'env> {
    primary: &'env ChainCasBuf<'env, Reader<'env>>,
    primary_meta: &'env ChainMetaBuf<'env, ()>,

    cache: &'env ChainCasBuf<'env, Reader<'env>>,
    cache_meta: &'env ChainMetaBuf<'env, ()>,
}

/// Should these functions be sync or async?
/// Depends on how much computation, and if writes are involved
impl<'env> Cascade<'env> {
    /// Take references to cas and cache
    pub fn new(
        primary: &'env ChainCasBuf<'env, Reader<'env>>,
        primary_meta: &'env ChainMetaBuf<'env, ()>,

        cache: &'env ChainCasBuf<'env, Reader<'env>>,
        cache_meta: &'env ChainMetaBuf<'env, ()>,
    ) -> Self {
        Cascade {
            primary,
            primary_meta,
            cache,
            cache_meta,
        }
    }
    pub async fn dht_get(&self, _address: Address) -> Entry {
        unimplemented!()
    }
}
