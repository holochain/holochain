//! An interface to an LMDB key-value store, with integer keys
//! This is unfortunately pure copypasta from KvBuffer, since Rust doesn't support specialization yet
//! TODO, find *some* way to DRY up the two

use super::{kv::SingleStoreIterTyped, BufferIntKey, BufferVal, StoreBuffer};
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
    K: BufferIntKey,
    V: BufferVal,
{
    db: IntegerStore<K>,
    reader: &'env Reader<'env>,
    scratch: HashMap<K, KvOp<V>>,
}

impl<'env, K, V> KvIntBuffer<'env, K, V>
where
    K: BufferIntKey,
    V: BufferVal,
{
    pub fn new(reader: &'env Reader<'env>, db: IntegerStore<K>) -> WorkspaceResult<Self> {
        Ok(Self {
            db,
            reader,
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
        match self.db.get(self.reader, k)? {
            Some(rkv::Value::Blob(buf)) => Ok(Some(rmp_serde::from_read_ref(buf)?)),
            None => Ok(None),
            Some(_) => Err(WorkspaceError::InvalidValue),
        }
    }

    pub fn iter_raw(&self) -> WorkspaceResult<SingleStoreIterTyped<V>> {
        Ok((SingleStoreIterTyped::new(self.db.iter_start(self.reader)?)))
    }

    pub fn iter_raw_reverse(&self) -> WorkspaceResult<SingleStoreIterTyped<V>> {
        Ok((SingleStoreIterTyped::new(self.db.iter_end(self.reader)?)))
    }
}

impl<'env, K, V> StoreBuffer<'env> for KvIntBuffer<'env, K, V>
where
    K: BufferIntKey,
    V: BufferVal,
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
    use crate::{
        db::{ReadManager, WriteManager},
        test_utils::test_env,
    };
    use rkv::StoreOptions;
    use serde_derive::{Deserialize, Serialize};

    #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
    struct TestVal {
        name: String,
    }

    #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
    struct V(u32);

    type Store<'a> = KvIntBuffer<'a, u32, V>;

    #[test]
    fn kv_iterators() {
        let arc = test_env();
        let env = arc.read().unwrap();
        let db = env.open_integer("kv", StoreOptions::create()).unwrap();
        let rm = ReadManager::new(&env);
        let wm = WriteManager::new(&env);

        rm.with_reader(|reader| {
            let mut buf: Store = KvIntBuffer::new(&reader, db).unwrap();

            buf.put(1, V(1));
            buf.put(2, V(2));
            buf.put(3, V(3));
            buf.put(4, V(4));
            buf.put(5, V(5));

            wm.with_writer(|mut writer| buf.finalize(&mut writer))
        })
        .unwrap();

        rm.with_reader(|reader| {
            let buf: Store = KvIntBuffer::new(&reader, db).unwrap();

            let forward: Vec<_> = buf.iter_raw().unwrap().collect();
            let reverse: Vec<_> = buf.iter_raw_reverse().unwrap().collect();

            assert_eq!(forward, vec![V(1), V(2), V(3), V(4), V(5)]);
            assert_eq!(reverse, vec![V(5), V(4), V(3), V(2), V(1)]);
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn kv_empty_iterators() {
        let arc = test_env();
        let env = arc.read().unwrap();
        let db = env.open_integer("kv", StoreOptions::create()).unwrap();
        let rm = ReadManager::new(&env);

        rm.with_reader(|reader| {
            let buf: Store = KvIntBuffer::new(&reader, db).unwrap();

            let forward: Vec<_> = buf.iter_raw().unwrap().collect();
            let reverse: Vec<_> = buf.iter_raw_reverse().unwrap().collect();

            assert_eq!(forward, vec![]);
            assert_eq!(reverse, vec![]);
            Ok(())
        })
        .unwrap();
    }
}
