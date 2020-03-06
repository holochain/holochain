use super::chain_cas::ChainCasBuffer;
use holochain_persistence_api::cas::content::Address;
use sx_state::{buffer::KvvBuffer, prelude::Reader};

struct Cascade<'e> {
    cas: &'e ChainCasBuffer<'e, Reader<'e>>,
    cache: &'e ChainCasBuffer<'e, Reader<'e>>,
    cas_meta: &'e KvvBuffer<'e, Address, ()>,
    cache_meta: &'e KvvBuffer<'e, Address, ()>,
}

impl<'env> Cascade<'env> {
    pub fn dht_get() {
        unimplemented!()
    }
}
