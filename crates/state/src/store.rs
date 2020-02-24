use crate::error::{WorkspaceError, WorkspaceResult};
use rkv::{Rkv, SingleStore, StoreOptions, Writer};
use serde::{de::DeserializeOwned, Serialize};
use std::{collections::HashMap, hash::Hash};

/// General trait for transactional stores, exposing only the method which
/// finalizes the transaction. Not currently used, but could be used in Workspaces
/// i.e. iterating over a Vec<dyn TransactionalStore> is all that needs to happen
/// to commit the workspace changes
pub trait TransactionalStore<'env>: Sized {
    fn finalize(self, writer: &'env mut Writer) -> WorkspaceResult<()>;
}

/// Transactional operations on a KV store
/// Add: add this KV if the key does not yet exist
/// Mod: set the key to this value regardless of whether or not it already exists
/// Del: remove the KV
enum KvOp<V> {
    Put(V),
    Del,
}

/// A persisted key-value store with a transient HashMap to store
/// CRUD-like changes without opening a blocking read-write cursor
///
/// TODO: split the various methods for accessing data into traits,
/// and write a macro to help produce traits for every possible combination
/// of access permission, so that access can be hidden behind a limited interface
pub struct KvStore<'env, K, V>
where
    K: Hash + Eq + AsRef<[u8]>,
    V: Clone + Serialize + DeserializeOwned,
{
    db: SingleStore,
    env: &'env Rkv,
    scratch: HashMap<K, KvOp<V>>,
}

impl<'env, K, V> KvStore<'env, K, V>
where
    K: Hash + Eq + AsRef<[u8]>,
    V: Clone + Serialize + DeserializeOwned,
{
    /// Create or open DB if it exists.
    /// CAREFUL with this! Calling create() during a transaction seems to cause a deadlock
    pub fn create(env: &'env Rkv, name: &str) -> WorkspaceResult<Self> {
        let db = env.open_single(name, StoreOptions::create())?;
        Ok(Self {
            db,
            env,
            scratch: HashMap::new(),
        })
    }

    /// Open an existing DB. Will cause an error if the DB was not created already.
    pub fn open(env: &'env Rkv, name: &str) -> WorkspaceResult<Self> {
        let db = env.open_single(name, StoreOptions::default())?;
        Ok(Self {
            db,
            env,
            scratch: HashMap::new(),
        })
    }

    pub fn get(&self, k: &K) -> WorkspaceResult<Option<V>> {
        use KvOp::*;
        let val = match self.scratch.get(k) {
            Some(Put(scratch_val)) => Some(scratch_val.clone()),
            Some(Del) => None,
            None => self.get_persisted(k)?,
        };
        Ok(val)
    }

    pub fn put(&mut self, k: K, v: V) {
        // TODO, maybe give indication of whether the value existed or not
        let _ = self.scratch.insert(k, KvOp::Put(v));
    }

    pub fn delete(&mut self, k: K) {
        // TODO, maybe give indication of whether the value existed or not
        let _ = self.scratch.insert(k, KvOp::Del);
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

impl<'env, K, V> TransactionalStore<'env> for KvStore<'env, K, V>
where
    K: Hash + Eq + AsRef<[u8]>,
    V: Clone + Serialize + DeserializeOwned,
{
    fn finalize(self, writer: &'env mut Writer) -> WorkspaceResult<()> {
        use KvOp::*;
        for (k, op) in self.scratch.iter() {
            match op {
                Put(v) => {
                    let buf = rmp_serde::to_vec_named(v)?;
                    let encoded = rkv::Value::Blob(&buf);
                    self.db.put(writer, k, &encoded)?;
                }
                Del => self.db.delete(writer, k)?,
            }
        }
        Ok(())
    }
}

/// Storage representing tabular data, useful for e.g. CAS metadata
/// This may be EAVI, but just a placeholder for now.
pub struct TabularStore;

impl<'env> TransactionalStore<'env> for TabularStore {
    fn finalize(self, writer: &'env mut Writer) -> WorkspaceResult<()> {
        unimplemented!()
    }
}

#[cfg(test)]
pub mod tests {

    use super::{KvStore, TransactionalStore};
    use crate::env::create_lmdb_env;
    use serde_derive::{Deserialize, Serialize};
    use tempdir::TempDir;

    #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
    struct TestVal {
        name: String,
    }

    #[test]
    fn kv_store_sanity_check() {
        let tmpdir = TempDir::new("skunkworx").unwrap();
        let created_arc = create_lmdb_env(tmpdir.path());
        let env = created_arc.read().unwrap();

        let mut kv1: KvStore<String, TestVal> = KvStore::create(&env, "kv1").unwrap();
        let mut kv2: KvStore<String, String> = KvStore::create(&env, "kv2").unwrap();

        let testval = TestVal {
            name: "Joe".to_owned(),
        };

        kv1.put(
            "hi".to_owned(),
            testval.clone(),
        );
        kv2.put("salutations".to_owned(), "folks".to_owned());

        // Check that the underlying store contains no changes yet
        assert_eq!(kv1.get_persisted(&"hi".to_owned()).unwrap(), None);
        assert_eq!(kv2.get_persisted(&"salutations".to_owned()).unwrap(), None);

        let mut writer = env.write().unwrap();
        kv1.finalize(&mut writer).unwrap();

        // Ensure that mid-transaction, there has still been no persistence,
        // just for kicks
        let kv1a: KvStore<String, TestVal> = KvStore::open(&env, "kv1").unwrap();
        assert_eq!(kv1a.get_persisted(&"hi".to_owned()).unwrap(), None);

        // Finish finalizing the transaction
        kv2.finalize(&mut writer).unwrap();
        writer.commit().unwrap();

        // Now open some fresh Readers to see that our data was persisted
        let kv1b: KvStore<String, TestVal> = KvStore::open(&env, "kv1").unwrap();
        let kv2b: KvStore<String, String> = KvStore::open(&env, "kv2").unwrap();
        // Check that the underlying store contains no changes yet
        assert_eq!(kv1b.get_persisted(&"hi".to_owned()).unwrap(), Some(testval));
        assert_eq!(kv2b.get_persisted(&"salutations".to_owned()).unwrap(), Some("folks".to_owned()));
    }
}
