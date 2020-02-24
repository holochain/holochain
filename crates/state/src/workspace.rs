use crate::{
    api::{Database, RoCursor, RwCursor, RwTransaction},
    error::{WorkspaceError, WorkspaceResult},
};
use rkv::{EnvironmentFlags, Manager, Reader, Rkv, SingleStore, StoreOptions, Writer};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::{
    collections::HashMap,
    hash::Hash,
    path::Path,
    sync::{Arc, RwLock},
};

pub trait Store<'env>: Sized {
    fn finalize(self, writer: &'env mut Writer) -> WorkspaceResult<()>;
}

enum KvCrud<V> {
    Add(V),
    Mod(V),
    Del,
}

/// A persisted key-value store with a transient HashMap to store
/// CRUD-like changes without opening a blocking read-write cursor
pub struct KvStore<'env, K, V>
where
    K: Hash + Eq + AsRef<[u8]>,
    V: Clone + Serialize + DeserializeOwned,
{
    db: SingleStore,
    env: &'env Rkv,
    // reader: Reader<'env>,
    scratch: HashMap<K, KvCrud<V>>,
}

impl<'env, K, V> KvStore<'env, K, V>
where
    K: Hash + Eq + AsRef<[u8]>,
    V: Clone + Serialize + DeserializeOwned,
{
    pub fn new(env: &'env Rkv, name: &str) -> WorkspaceResult<Self> {
        let db = env.open_single(name, StoreOptions::create())?;
        Ok(Self {
            db,
            env,
            scratch: HashMap::new(),
        })
    }

    pub fn get(&self, k: &K) -> WorkspaceResult<Option<V>> {
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

    pub fn add(&mut self, k: K, v: V) {
        // TODO, maybe give indication of whether the value existed or not
        let _ = self.scratch.insert(k, KvCrud::Add(v));
    }

    pub fn modify(&mut self, k: K, v: V) {
        // TODO, maybe give indication of whether the value existed or not
        let _ = self.scratch.insert(k, KvCrud::Mod(v));
    }

    pub fn delete(&mut self, k: &K) {
        // TODO, maybe give indication of whether the value existed or not
        let _ = self.scratch.remove(k);
    }

    /// Fetch data from DB, deserialize into V type
    fn get_persisted(&self, k: &K) -> WorkspaceResult<Option<V>> {
        match self.db.get(&self.env.read()?, k)? {
            Some(rkv::Value::Blob(buf)) => Ok(Some(rmp_serde::from_read_ref(buf)?)),
            None => Ok(None),
            Some(_) => Err(WorkspaceError::InvalidValue),
        }
    }
}

// fn rkv_encode<'a, V>(v: &'a V) -> WorkspaceResult<rkv::Value<'a>>
// where V: Serialize
// {
//     let buf = rmp_serde::to_vec_named(v)?;
//     Ok(rkv::Value::from_tagged_slice(&buf)?)
// }

impl<'env, K, V> Store<'env> for KvStore<'env, K, V>
where
    K: Hash + Eq + AsRef<[u8]>,
    V: Clone + Serialize + DeserializeOwned,
{
    fn finalize(self, writer: &'env mut Writer) -> WorkspaceResult<()> {
        use KvCrud::*;
        for (k, op) in self.scratch.iter() {
            match op {
                Add(v) | Mod(v) => {
                    let buf = rmp_serde::to_vec_named(v)?;
                    let encoded = rkv::Value::from_tagged_slice(&buf)?;
                    match op {
                        Add(_) => {
                            if self.get_persisted(&k)?.is_none() {
                                self.db.put(writer, k, &encoded)?;
                            }
                        }
                        Mod(_) => self.db.put(writer, k, &encoded)?,
                        Del => unreachable!(),
                    }
                }
                Del => self.db.delete(writer, k)?,
            }
        }
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
    pub fn new(env: &'env Rkv) -> WorkspaceResult<Self> {
        Ok(Self {
            cas: KvStore::new(env, "cas")?,
            meta: TabularStore,
        })
    }

    pub fn cas(&mut self) -> &mut KvStore<'env, String, String> {
        &mut self.cas
    }
}

impl<'env> Workspace<'env> for AppValidationWorkspace {
    fn finalize(self, writer: Writer) -> WorkspaceResult<()> {
        Ok(())
    }
}

const DEFAULT_INITIAL_MAP_SIZE: usize = 100 * 1024 * 1024;
const MAX_DBS: u32 = 32;

fn create_rkv_env(path: &Path) -> Arc<RwLock<Rkv>> {
    let initial_map_size = None;
    let flags = None;
    Manager::singleton()
        .write()
        .unwrap()
        .get_or_create(path, |path: &Path| {
            let mut env_builder = Rkv::environment_builder();
            env_builder
                // max size of memory map, can be changed later
                .set_map_size(initial_map_size.unwrap_or(DEFAULT_INITIAL_MAP_SIZE))
                // max number of DBs in this environment
                .set_max_dbs(MAX_DBS)
                // These flags make writes waaaaay faster by async writing to disk rather than blocking
                // There is some loss of data integrity guarantees that comes with this
                .set_flags(
                    flags.unwrap_or_else(|| {
                        EnvironmentFlags::WRITE_MAP | EnvironmentFlags::MAP_ASYNC
                    }),
                );
            Rkv::from_env(path, env_builder)
        })
        .unwrap()
}

#[cfg(test)]
pub mod tests {

    use super::{create_rkv_env, InvokeZomeWorkspace};
    use rkv::Rkv;
    use tempdir::TempDir;

    #[test]
    fn create_invoke_zome_workspace() {
        let tmpdir = TempDir::new("skunkworx").unwrap();
        println!("temp dir: {:?}", tmpdir);
        let created_arc = create_rkv_env(tmpdir.path());
        let env = created_arc.read().unwrap();
        let mut workspace = InvokeZomeWorkspace::new(&env).unwrap();
        let cas = workspace.cas();
        assert_eq!(cas.get(&"hi".to_owned()).unwrap(), None);
        cas.add("hi".to_owned(), "there".to_owned());
        assert_eq!(cas.get(&"hi".to_owned()).unwrap(), Some("there".to_owned()));
    }
}
