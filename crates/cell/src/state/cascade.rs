use super::chain_cas::ChainCasBuffer;
use holochain_persistence_api::cas::content::Address;
use sx_state::buffer::KvvBuffer;

struct Cascade<'e> {
    cas: &'e ChainCasBuffer<'e>,
    cache: &'e ChainCasBuffer<'e>,
    cas_meta: &'e KvvBuffer<'e, Address, ()>,
    cache_meta: &'e KvvBuffer<'e, Address, ()>,
}

impl<'env> Cascade<'env> {
    pub fn dht_get() {
        unimplemented!()
    }
}
