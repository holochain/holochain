use crate::buffer::{cas::CasBuffer, kvv::KvvBuffer};
use sx_types::{chain_header::ChainHeader, entry::Entry};

struct Cascade<'e> {
    entry_cas: &'e CasBuffer<'e, Entry>,
    entry_cas_cache: &'e CasBuffer<'e, Entry>,
    header_cas: &'e CasBuffer<'e, ChainHeader>,
    header_cas_cache: &'e CasBuffer<'e, ChainHeader>,
    cas_meta: &'e KvvBuffer<'e, String, String>,
    cache_meta: &'e KvvBuffer<'e, String, String>,
}

impl<'env> Cascade<'env> {
    pub fn dht_get() {
        unimplemented!()
    }
}
