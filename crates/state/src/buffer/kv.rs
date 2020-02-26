use super::StoreBuffer;
use crate::error::{WorkspaceError, WorkspaceResult};
use rkv::{Reader, Rkv, SingleStore, StoreOptions, Writer};
use serde::{de::DeserializeOwned, Serialize};
use std::{collections::HashMap, hash::Hash};

/// Transactional operations on a KV store
/// Add: add this KV if the key does not yet exist
/// Mod: set the key to this value regardless of whether or not it already exists
/// Del: remove the KV
enum KvOp<V> {
    Put(Box<V>),
    Del,
}

/// A persisted key-value store with a transient HashMap to store
/// CRUD-like changes without opening a blocking read-write cursor
///
/// TODO: split the various methods for accessing data into traits,
/// and write a macro to help produce traits for every possible combination
/// of access permission, so that access can be hidden behind a limited interface
///
/// TODO: hold onto SingleStore references for as long as the env
pub struct KvBuffer<'env, K, V>
where
    K: Hash + Eq + AsRef<[u8]>,
    V: Clone + Serialize + DeserializeOwned,
{
    db: SingleStore,
    reader: Reader<'env>,
    scratch: HashMap<K, KvOp<V>>,
}

impl<'env, K, V> KvBuffer<'env, K, V>
where
    K: Hash + Eq + AsRef<[u8]>,
    V: Clone + Serialize + DeserializeOwned,
{
    /// Create or open DB if it exists.
    /// CAREFUL with this! Calling create() during a transaction seems to cause a deadlock
    pub fn create(env: &'env Rkv, name: &str) -> WorkspaceResult<Self> {
        Ok(Self {
            db: env.open_single(name, StoreOptions::create())?,
            reader: env.read()?,
            scratch: HashMap::new(),
        })
    }

    /// Open an existing DB. Will cause an error if the DB was not created already.
    pub fn open(env: &'env Rkv, name: &str) -> WorkspaceResult<Self> {
        Ok(Self {
            db: env.open_single(name, StoreOptions::default())?,
            reader: env.read()?,
            scratch: HashMap::new(),
        })
    }

    pub fn get(&self, k: &K) -> WorkspaceResult<Option<V>> {
        use KvOp::*;
        let val = match self.scratch.get(k) {
            Some(Put(scratch_val)) => Some(*scratch_val.clone()),
            Some(Del) => None,
            None => self.get_persisted(k)?,
        };
        Ok(val)
    }

    pub fn put(&mut self, k: K, v: V) {
        // TODO, maybe give indication of whether the value existed or not
        let _ = self.scratch.insert(k, KvOp::Put(Box::new(v)));
    }

    pub fn delete(&mut self, k: K) {
        // TODO, maybe give indication of whether the value existed or not
        let _ = self.scratch.insert(k, KvOp::Del);
    }

    /// Fetch data from DB, deserialize into V type
    fn get_persisted(&self, k: &K) -> WorkspaceResult<Option<V>> {
        match self.db.get(&self.reader, k)? {
            Some(rkv::Value::Blob(buf)) => Ok(Some(rmp_serde::from_read_ref(buf)?)),
            None => Ok(None),
            Some(_) => Err(WorkspaceError::InvalidValue),
        }
    }

    fn iter(&self) -> WorkspaceResult<SingleStoreIterTyped<V>> {
        Ok((SingleStoreIterTyped::new(self.db.iter_start(&self.reader)?)))
    }

    fn iter_reverse(&self) -> WorkspaceResult<SingleStoreIterTyped<V>> {
        Ok((SingleStoreIterTyped::new(self.db.iter_end(&self.reader)?)))
    }
}

impl<'env, K, V> StoreBuffer<'env, K, V> for KvBuffer<'env, K, V>
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

struct SingleStoreIterTyped<'env, V>(rkv::store::single::Iter<'env>, std::marker::PhantomData<V>);

impl<'env, V> SingleStoreIterTyped<'env, V> {
    pub fn new(iter: rkv::store::single::Iter<'env>) -> Self {
        Self(iter, std::marker::PhantomData)
    }
}


impl<'env, V> Iterator for SingleStoreIterTyped<'env, V>
where V: Clone + Serialize + DeserializeOwned, {
    type Item = V;

    fn next(&mut self) -> Option<Self::Item> {
        match self.0.next() {
            Some(Ok((_k, Some(rkv::Value::Blob(buf))))) => (
                // k.into(),
                rmp_serde::from_read_ref(buf).unwrap()
            ),
            None => None,
            x => {
                dbg!(x);
                panic!("TODO");
            }
        }
    }
}

#[cfg(test)]
pub mod tests {

    use super::{KvBuffer, StoreBuffer};
    use crate::env::{create_lmdb_env, test::{with_writer, test_env}};
    use serde_derive::{Deserialize, Serialize};
    use tempdir::TempDir;

    #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
    struct TestVal {
        name: String,
    }

    #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
    struct V(u32);

    #[test]
    fn kv_iterators() {
        let arc = test_env();
        let env = arc.read().unwrap();
        type Store<'a> = KvBuffer<'a, &'a str, V>;

        let mut store: Store = KvBuffer::create(&env, "kv").unwrap();

        store.put("a", V(1));
        store.put("b", V(2));
        store.put("c", V(3));
        store.put("d", V(4));
        store.put("e", V(5));

        with_writer(&env, |mut writer| store.finalize(&mut writer));

        let store: Store = KvBuffer::open(&env, "kv").unwrap();

        let forward: Vec<_> = store.iter().unwrap().collect();
        let reverse: Vec<_> = store.iter_reverse().unwrap().collect();

        assert_eq!(forward, vec![V(1), V(2), V(3), V(4), V(5)]);
        assert_eq!(reverse, vec![V(5), V(4), V(3), V(2), V(1)]);
    }

    #[test]
    fn kv_empty_iterators() {
        let arc = test_env();
        let env = arc.read().unwrap();
        type Store<'a> = KvBuffer<'a, &'a str, V>;

        let store: Store = KvBuffer::create(&env, "kv").unwrap();

        let forward: Vec<_> = store.iter().unwrap().collect();
        let reverse: Vec<_> = store.iter_reverse().unwrap().collect();

        assert_eq!(forward, vec![]);
        assert_eq!(reverse, vec![]);
    }

    /// TODO break up into smaller tests
    #[test]
    fn kv_store_sanity_check() {
        let arc = test_env();
        let env = arc.read().unwrap();

        let mut kv1: KvBuffer<String, TestVal> = KvBuffer::create(&env, "kv1").unwrap();
        let mut kv2: KvBuffer<String, String> = KvBuffer::create(&env, "kv2").unwrap();

        let testval = TestVal {
            name: "Joe".to_owned(),
        };

        kv1.put("hi".to_owned(), testval.clone());
        kv2.put("salutations".to_owned(), "folks".to_owned());

        // Check that the underlying store contains no changes yet
        assert_eq!(kv1.get_persisted(&"hi".to_owned()).unwrap(), None);
        assert_eq!(kv2.get_persisted(&"salutations".to_owned()).unwrap(), None);

        let mut writer = env.write().unwrap();
        kv1.finalize(&mut writer).unwrap();

        // Ensure that mid-transaction, there has still been no persistence,
        // just for kicks
        let kv1a: KvBuffer<String, TestVal> = KvBuffer::open(&env, "kv1").unwrap();
        assert_eq!(kv1a.get_persisted(&"hi".to_owned()).unwrap(), None);

        // Finish finalizing the transaction
        kv2.finalize(&mut writer).unwrap();
        writer.commit().unwrap();

        // Now open some fresh Readers to see that our data was persisted
        let kv1b: KvBuffer<String, TestVal> = KvBuffer::open(&env, "kv1").unwrap();
        let kv2b: KvBuffer<String, String> = KvBuffer::open(&env, "kv2").unwrap();
        // Check that the underlying store contains no changes yet
        assert_eq!(kv1b.get_persisted(&"hi".to_owned()).unwrap(), Some(testval));
        assert_eq!(
            kv2b.get_persisted(&"salutations".to_owned()).unwrap(),
            Some("folks".to_owned())
        );
    }
}
