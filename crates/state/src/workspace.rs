use crate::{
    api::{Database, RoCursor, RwCursor, RwTransaction},
    error::WorkspaceResult,
};
use rkv::{Reader, SingleStore, StoreOptions, Writer, Rkv};
use std::{collections::HashMap, hash::Hash};

pub trait Store<'env>: Sized {
    fn finalize(self, writer: &'env mut Writer) -> WorkspaceResult<()>;
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
pub struct KvStore<'env, K: Hash + Eq, V> {
    db: SingleStore,
    reader: Reader<'env>,
    scratch: HashMap<K, KvCrud<V>>,
}

impl<'env, K: Hash + Eq, V: Clone> KvStore<'env, K, V> {

    pub fn new(env: &'env Rkv, name: &str) -> Self {
        let reader = env.read().expect("TODO");
        let db = env.open_single(name, StoreOptions::create()).expect("TODO");
        Self {
            db,
            reader,
            scratch: HashMap::new()
        }
    }

    pub fn get(&self, k: &K) -> StateResult<Option<V>> {
        use KvCrud::*;
        let val = match self.scratch.get(k) {
            Some(Add(scratch_val)) => Some(
                self.get_persisted(k)?
                    .unwrap_or_else(|| scratch_val.clone()),
            ),
            Some(Mod(scratch_val)) => Some(scratch_val.clone()),
            Some(Del) => None,
            None => self.get_persisted(k)?,
        };
        Ok(val)
    }

    /// Fetch data from DB, deserialize into V type
    fn get_persisted(&self, k: &K) -> StateResult<Option<V>> {
        Ok(self.db.get(&self.reader, k).expect("TODO"))
    }
}

impl<'env, K: Hash + Eq, V> Store<'env> for KvStore<'env, K, V> {
    fn finalize(self, writer: &'env mut Writer) -> WorkspaceResult<()> {
        // TODO: iterate over scratch values and apply them to the cursor
        // via db.put / db.delete
        unimplemented!()
    }
}

struct TabularStore;
impl<'env> Store<'env> for TabularStore {
    fn finalize(self, writer: &'env mut Writer) -> WorkspaceResult<()> {
        unimplemented!()
    }
}

struct Cascade<'env> {
    cas: &'env SingleStore,
    cas_meta: &'env SingleStore,
    cache: &'env SingleStore,
    cache_meta: &'env SingleStore,
}

pub trait Workspace<'txn>: Sized {
    fn finalize(self, writer: Writer) -> WorkspaceResult<()>;
}

pub struct InvokeZomeWorkspace<'env> {
    cas: KvStore<'env, String, String>,
    meta: TabularStore,
}

/// There can be a different set of db cursors (all writes) that only get accessed in the finalize stage,
/// but other read-only cursors during the actual workflow
pub struct AppValidationWorkspace;

impl<'env> Workspace<'env> for InvokeZomeWorkspace<'env> {
    fn finalize(self, mut writer: Writer) -> WorkspaceResult<()> {
        self.cas.finalize(&mut writer)?;
        self.meta.finalize(&mut writer)?;
        Ok(())
    }
}

impl<'env> InvokeZomeWorkspace<'env> {
    pub fn new(env: &'env Rkv) -> Self {
        Self {
            cas: KvStore::new(env, "cas"),
            meta: TabularStore,
        }
    }

    pub fn cas(&self) -> &KvStore<String, String> {
        &self.cas
    }
}

impl<'env> Workspace<'env> for AppValidationWorkspace {
    fn finalize(self, writer: Writer) -> WorkspaceResult<()> {
        Ok(())
    }
}

#[cfg(test)]
pub mod tests {

    use tempdir::TempDir;
    use rkv::{Rkv, Manager};
    use super::InvokeZomeWorkspace;

    #[test]
    fn create_invoke_zome_workspace() {
        let tmpdir = TempDir::new("skunkworx").unwrap();
        let created_arc = Manager::singleton().write().unwrap().get_or_create(tmpdir.path(), Rkv::new).unwrap();
        let env = created_arc.read().unwrap();
        let workspace = InvokeZomeWorkspace::new(&env);
    }
}
