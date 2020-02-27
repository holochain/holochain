//! An interface to an LMDB key-value store, with integer keys
//! This is unfortunately pure copypasta from KvBuffer, since Rust doesn't support specialization yet
//! TODO, find *some* way to DRY up the two

use super::{kv::SingleStoreIterTyped, StoreBuffer};
use crate::error::{WorkspaceError, WorkspaceResult};
use rkv::{IntegerStore, Reader, Rkv, StoreOptions, Writer};
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
pub struct KvIntBuffer<'env, K, V>
where
    K: Hash + Eq + rkv::store::integer::PrimitiveInt,
    V: Clone + Serialize + DeserializeOwned,
{
    db: IntegerStore<K>,
    reader: Reader<'env>,
    scratch: HashMap<K, KvOp<V>>,
}

impl<'env, K, V> KvIntBuffer<'env, K, V>
where
    K: Hash + Eq + rkv::store::integer::PrimitiveInt,
    V: Clone + Serialize + DeserializeOwned,
{
    /// Create or open DB if it exists.
    /// CAREFUL with this! Calling create() during a transaction seems to cause a deadlock
    pub fn create(env: &'env Rkv, name: &str) -> WorkspaceResult<Self> {
        Ok(Self {
            db: env.open_integer(name, StoreOptions::create())?,
            reader: env.read()?,
            scratch: HashMap::new(),
        })
    }

    /// Open an existing DB. Will cause an error if the DB was not created already.
    pub fn open(env: &'env Rkv, name: &str) -> WorkspaceResult<Self> {
        Ok(Self {
            db: env.open_integer(name, StoreOptions::default())?,
            reader: env.read()?,
            scratch: HashMap::new(),
        })
    }

    pub fn get(&self, k: K) -> WorkspaceResult<Option<V>> {
        use KvOp::*;
        let val = match self.scratch.get(&k) {
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
    fn get_persisted(&self, k: K) -> WorkspaceResult<Option<V>> {
        match self.db.get(&self.reader, k)? {
            Some(rkv::Value::Blob(buf)) => Ok(Some(rmp_serde::from_read_ref(buf)?)),
            None => Ok(None),
            Some(_) => Err(WorkspaceError::InvalidValue),
        }
    }

    pub fn iter(&self) -> WorkspaceResult<SingleStoreIterTyped<V>> {
        Ok((SingleStoreIterTyped::new(self.db.iter_start(&self.reader)?)))
    }

    pub fn iter_reverse(&self) -> WorkspaceResult<SingleStoreIterTyped<V>> {
        Ok((SingleStoreIterTyped::new(self.db.iter_end(&self.reader)?)))
    }
}

impl<'env, K, V> StoreBuffer<'env, K, V> for KvIntBuffer<'env, K, V>
where
    K: Hash + Eq + rkv::store::integer::PrimitiveInt,
    V: Clone + Serialize + DeserializeOwned,
{
    fn finalize(self, writer: &'env mut Writer) -> WorkspaceResult<()> {
        use KvOp::*;
        for (k, op) in self.scratch.iter() {
            match op {
                Put(v) => {
                    let buf = rmp_serde::to_vec_named(v)?;
                    let encoded = rkv::Value::Blob(&buf);
                    self.db.put(writer, *k, &encoded)?;
                }
                Del => self.db.delete(writer, *k)?,
            }
        }
        Ok(())
    }
}

#[cfg(test)]
pub mod tests {

    use super::{KvIntBuffer, StoreBuffer};
    use crate::env::test::{test_env, with_writer};
    use serde_derive::{Deserialize, Serialize};

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
        type Store<'a> = KvIntBuffer<'a, u32, V>;

        let mut store: Store = KvIntBuffer::create(&env, "kv").unwrap();

        store.put(1, V(1));
        store.put(2, V(2));
        store.put(3, V(3));
        store.put(4, V(4));
        store.put(5, V(5));

        with_writer(&env, |mut writer| store.finalize(&mut writer));

        let store: Store = KvIntBuffer::open(&env, "kv").unwrap();

        let forward: Vec<_> = store.iter().unwrap().collect();
        let reverse: Vec<_> = store.iter_reverse().unwrap().collect();

        assert_eq!(forward, vec![V(1), V(2), V(3), V(4), V(5)]);
        assert_eq!(reverse, vec![V(5), V(4), V(3), V(2), V(1)]);
    }

    #[test]
    fn kv_empty_iterators() {
        let arc = test_env();
        let env = arc.read().unwrap();
        type Store<'a> = KvIntBuffer<'a, u32, V>;

        let store: Store = KvIntBuffer::create(&env, "kv").unwrap();

        let forward: Vec<_> = store.iter().unwrap().collect();
        let reverse: Vec<_> = store.iter_reverse().unwrap().collect();

        assert_eq!(forward, vec![]);
        assert_eq!(reverse, vec![]);
    }
}
