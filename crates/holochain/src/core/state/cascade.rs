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

use sx_types::entry::Entry;
use super::chain_cas::ChainCasBuf;
use sx_types::persistence::cas::content::Address;
use sx_state::{buffer::KvvBuf, prelude::Reader};

#[allow(dead_code)]
pub struct Cascade<'e> {
    cas: &'e ChainCasBuf<'e, Reader<'e>>,
    cas_meta: &'e KvvBuf<'e, Address, ()>,

    cache: &'e ChainCasBuf<'e, Reader<'e>>,
    cache_meta: &'e KvvBuf<'e, Address, ()>,
}

/// Should these functions be sync or async?
/// Depends on how much computation, and if writes are involved
impl<'env> Cascade<'env> {
    pub async fn get(address: Address) -> Entry {
        unimplemented!()
    }
    pub async fn dht_get() {
        unimplemented!()
    }

    pub async fn dht_get_links() {
        unimplemented!()
    }
}