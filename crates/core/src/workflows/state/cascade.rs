use super::chain_cas::ChainCasBuf;
use holochain_persistence_api::cas::content::Address;
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
    pub async fn dht_get() {
        unimplemented!()
    }

    pub async fn dht_get_links() {
        unimplemented!()
    }
}
