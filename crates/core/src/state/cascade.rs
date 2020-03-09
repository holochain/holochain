use super::chain_cas::ChainCasBuf;
use holochain_persistence_api::cas::content::Address;
use sx_state::{buffer::KvvBuf, prelude::Reader};

#[allow(dead_code)]
pub struct Cascade<'e> {
    cas: &'e ChainCasBuf<'e, Reader<'e>>,
    cache: &'e ChainCasBuf<'e, Reader<'e>>,
    cas_meta: &'e KvvBuf<'e, Address, ()>,
    cache_meta: &'e KvvBuf<'e, Address, ()>,
}

impl<'env> Cascade<'env> {
    pub fn dht_get() {
        unimplemented!()
    }
}
