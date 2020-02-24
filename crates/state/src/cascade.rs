
use rkv::SingleStore;

struct Cascade<'env> {
    cas: &'env SingleStore,
    cas_meta: &'env SingleStore,
    cache: &'env SingleStore,
    cache_meta: &'env SingleStore,
}
