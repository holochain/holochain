use sx_state::buffer::{KvvBuffer};
use super::chain_cas::ChainCasBuffer;

struct Cascade<'e> {
    cas: &'e ChainCasBuffer<'e>,
    cache: &'e ChainCasBuffer<'e>,
    cas_meta: &'e KvvBuffer<'e, String, String>,
    cache_meta: &'e KvvBuffer<'e, String, String>,
}

impl<'env> Cascade<'env> {
    pub fn dht_get() {
        unimplemented!()
    }
}
