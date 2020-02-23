

use crate::{api::{RwTransaction, RwCursor, Database, RoCursor}, error::WorkspaceResult};
use std::collections::HashMap;

pub trait Workspace<'txn>: Sized {
    fn finalize(self) -> WorkspaceResult<()>;
}

pub trait Store: Sized {
    fn finalize(self) -> WorkspaceResult<()>;
}

enum KvCrud<V> {
    Add(V),
    Mod(V),
    Del,
}


// TODO: make real
pub type StateResult<T> = Result<T, ()>;

/// A persisted key-value store with a transient HashMap to store
/// CRUD-like changes without opening a blocking read-write cursor
struct KvStore<'txn, K, V> {
    db: Database,
    cursor: RoCursor<'txn>,
    scratch: HashMap<K, KvCrud<V>>,
}


impl<'txn, K, V> KvStore<'txn, K, V> {
    pub fn get(&self, k: &K) -> StateResult<Option<V>> {
        use KvCrud::*;
        match self.scratch.get(k) {
            Some(Add(val)) => self.get_persisted(k).unwrap_or(val),
            Some(Mod(val)) => Some(val),
            Some(Del) => None,
            None => self.get_persisted(k)?
        }
    }

    /// Fetch data from DB, deserialize into V type
    fn get_persisted(&self, k: &K) -> StateResult<Option<V>> {
        unimplemented!()
    }
}

impl<'txn, K, V> Store for KvStore<'txn, K, V> {
    fn finalize(self, txn: &'txn mut RwTransaction) -> WorkspaceResult<()> {
        let cursor = txn.open_rw_cursor(self.db).expect("TODO");
        // TODO: iterate over scratch values and apply them to the cursor
        unimplemented!()
    }
}

struct TabularStore;
impl Store for TabularStore {
    fn finalize(self) -> WorkspaceResult<()> {
        unimplemented!()
    }
}

struct Cascade<'txn> {
    cas: RwCursor<'txn>,
    cas_meta: RwCursor<'txn>,
    cache: RwCursor<'txn>,
    cache_meta: RwCursor<'txn>,
}

pub struct InvokeZomeWorkspace {
    cas: KvStore,
    meta: TabularStore,
}

/// There can be a different set of db cursors (all writes) that only get accessed in the finalize stage,
/// but other read-only cursors during the actual workflow
pub struct AppValidationWorkspace;

impl Workspace for InvokeZomeWorkspace {
    fn finalize(self) -> WorkspaceResult<()> {
        self.cas.finalize()?;
        self.meta.finalize()?;
        Ok(())
    }
}

impl Workspace for AppValidationWorkspace {
    fn finalize(self) -> WorkspaceResult<()> {
        Ok(())
    }
}
