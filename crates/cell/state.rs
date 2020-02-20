

pub struct HolochainState {
    LMDB MAGICAL THINGS HERE
}

////////////////////////////////////
// pseudocode musings on persistence

pub enum StoreName {
    Cas,
    CasMeta,
    PrivateEntries,
    CasCache,
    // ...etc
}

impl From<StoreName> for String {
    fn from(_: StoreName) -> String {
        unimplemented!()
    }
}

struct Lmdb<K, V> {
    name: String,
}

impl Lmdb {
    fn get(key: K) -> Option<V>;
}

struct ScratchSpace<K, V> {
    main: Lmdb<K, V>,
    scratch: Lmdb<K, V>,
}

impl ScratchSpace {
    get()
    add()
    update()
    delete()
    // get_links()
}

struct PersistenceQuery<K, V> {
    stores: Vec<Lmdb<K, V>>
}

impl<K, V> PersistenceQuery<K, V> {
    pub fn cascading_get(key: K) -> Option<V> {
        for store in stores {
            if let Some(val) = store.get(key) {
                return Some(val)
            }
        }
        return None
    }
}

// cas meta EAV: E = base, A = link, V = target

struct DhtOpIntegrationRecord {
    op_code: DhtOp,
    entry_hash: ChainAddress,
    header_hash: ChainAddress,
    authority_address: DhtAddress,
    when_integrated: Instant,
}

enum DhtOp;

// pub type DhtOpIntegrationRecordStore = ScratchSpace<Address, DhtOpIntegrationRecord>;
pub type CasStore = ScratchSpace<Address, Content>;

struct AppValidationWorkspace {
    // write access to a DhtOpIntegrationStore
    dht_op_integration_records: ScratchSpace<Address, DhtOpIntegrationRecord>,
    // cascade:
}

trait Add<K, V> {
    fn add(key: K, val: V);
}

trait Modify<K, V> {
    fn modify(key: K, val: V);
}

trait Del<K> {
    fn del(key: K);
}

Add<K, V> + Modify<K, V>
