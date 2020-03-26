use super::chain_cas::ChainCasBuf;
use sx_state::{buffer::KvvBuf, prelude::Reader};
use sx_types::persistence::cas::content::Address;

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
