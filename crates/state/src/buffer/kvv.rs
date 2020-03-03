use super::{BufferKey, BufferMultiVal, StoreBuffer};
use crate::error::{WorkspaceError, WorkspaceResult};
use maplit::hashset;
use rkv::{MultiStore, Reader, Rkv, StoreOptions, Writer};

use std::{
    collections::{hash_map::Entry, HashMap, HashSet},
    hash::Hash,
};

/// Transactional operations on a KVV store
/// Replace is a Delete followed by an Insert
#[derive(Debug, Hash, PartialEq, Eq)]
enum Op<V> {
    Insert(Box<V>),
    // Replace(Box<(V, V)>),
    Delete(Box<V>),
    DeleteAll,
}

type Scratch<K, V> = HashMap<K, HashSet<Op<V>>>;

/// A persisted key-value store with a transient HashMap to store
/// CRUD-like changes without opening a blocking read-write cursor
///
/// TODO: split the various methods for accessing data into traits,
/// and write a macro to help produce traits for every possible combination
/// of access permission, so that access can be hidden behind a limited interface
pub struct KvvBuffer<'env, K, V>
where
    K: BufferKey,
    V: BufferMultiVal,
{
    db: MultiStore,
    reader: Reader<'env>,
    scratch: Scratch<K, V>,
}

impl<'env, K, V> KvvBuffer<'env, K, V>
where
    K: BufferKey,
    V: BufferMultiVal,
{
    // TODO: restructure to match the others
    /// Create or open DB if it exists.
    /// CAREFUL with this! Calling create() during a transaction seems to cause a deadlock
    pub fn create(env: &'env Rkv, name: &str) -> WorkspaceResult<Self> {
        Ok(Self {
            db: env.open_multi(name, StoreOptions::create())?,
            reader: env.read()?,
            scratch: HashMap::new(),
        })
    }

    /// Open an existing DB. Will cause an error if the DB was not created already.
    pub fn open(env: &'env Rkv, name: &str) -> WorkspaceResult<Self> {
        Ok(Self {
            db: env.open_multi(name, StoreOptions::default())?,
            reader: env.read()?,
            scratch: HashMap::new(),
        })
    }

    pub fn get(&self, k: &K) -> WorkspaceResult<HashSet<V>> {
        use Op::*;
        let mut values = self.get_persisted(k)?;
        if let Some(ops) = self.scratch.get(k) {
            for op in ops.into_iter() {
                let _ = match op {
                    Insert(v) => values.insert(*v.clone()),
                    Delete(v) => values.remove(&**v),
                    DeleteAll => {
                        let _ = values.drain();
                        true
                    }
                };
            }
        }
        Ok(values)
    }

    pub fn insert(&mut self, k: K, v: V) {
        self.scratch
            .entry(k)
            .and_modify(|ops| {
                ops.remove(&Op::Delete(Box::new(v.clone())));
                let _ = ops.insert(Op::Insert(Box::new(v.clone())));
            })
            .or_insert_with(|| hashset! { Op::Insert(Box::new(v)) });
    }

    pub fn delete(&mut self, k: K, v: V) {
        // let deletion = Op::Delete(Box::new(v));
        self.scratch
            .entry(k)
            .and_modify(|ops| {
                ops.remove(&Op::Insert(Box::new(v.clone())));
                let _ = ops.insert(Op::Delete(Box::new(v.clone())));
            })
            .or_insert_with(|| hashset! { Op::Delete(Box::new(v)) });
    }

    pub fn delete_all(&mut self, k: K) {
        if let Entry::Occupied(mut entry) = self.scratch.entry(k) {
            let _ops = entry.get_mut();
        }
    }

    /// Fetch data from DB, deserialize into V type
    fn get_persisted(&self, k: &K) -> WorkspaceResult<HashSet<V>> {
        let iter = self.db.get(&self.reader, k)?;
        Ok(iter
            .map(|v| match v {
                Ok((_, Some(rkv::Value::Blob(buf)))) => Ok(Some(rmp_serde::from_read_ref(buf)?)),
                Ok((_, Some(_))) => Err(WorkspaceError::InvalidValue),
                Ok((_, None)) => Ok(None),
                Err(e) => Ok(Err(e)?),
                // Err(e) => match e {
                //     rkv::StoreError::LmdbError(lmdb::Error::NotFound) => Ok(None),
                //     e => Ok(Err(e)?)
                // },
            })
            .collect::<Result<Vec<Option<V>>, WorkspaceError>>()?
            .into_iter()
            .filter_map(|v| v)
            .collect())
    }
}

impl<'env, K, V> StoreBuffer<'env> for KvvBuffer<'env, K, V>
where
    K: Clone + BufferKey,
    V: BufferMultiVal,
{
    fn finalize(self, writer: &'env mut Writer) -> WorkspaceResult<()> {
        use Op::*;
        for (k, mut ops) in self.scratch.into_iter() {
            // If there is a DeleteAll in the set, that signifies that we should
            // delete everything persisted, but then continue to add inserts from
            // the ops, if present
            if ops.take(&DeleteAll).is_some() {
                self.db.delete_all(writer, k.clone())?;
            }
            for op in ops {
                match op {
                    Insert(v) => {
                        let buf = rmp_serde::to_vec_named(&*v)?;
                        let encoded = rkv::Value::Blob(&buf);
                        self.db.put(writer, k.clone(), &encoded)?;
                    }
                    Delete(v) => {
                        let buf = rmp_serde::to_vec_named(&*v)?;
                        let encoded = rkv::Value::Blob(&buf);
                        self.db.delete(writer, k.clone(), &encoded).or_else(|err| {
                            // Ignore the case where the key is not found
                            if let rkv::StoreError::LmdbError(lmdb::Error::NotFound) = err {
                                Ok(())
                            } else {
                                Err(err)
                            }
                        })?;
                    }
                    DeleteAll => unreachable!(),
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
pub mod tests {

    use super::{KvvBuffer, Op, StoreBuffer};
    use crate::{db::WriteManager, test_utils::test_env};
    use maplit::hashset;
    use rkv::Rkv;
    use serde_derive::{Deserialize, Serialize};
    
    

    #[derive(Clone, Debug, Hash, PartialEq, Eq, Serialize, Deserialize)]
    struct V(pub u32);

    type Store<'a> = KvvBuffer<'a, &'a str, V>;

    fn opInsert<T>(v: T) -> Op<T> {
        Op::Insert(Box::new(v))
    }

    fn opDelete<T>(v: T) -> Op<T> {
        Op::Delete(Box::new(v))
    }

    #[test]
    fn kvv_store_scratch_insert_delete() {
        let arc = test_env();
        let env = arc.read().unwrap();
        let wm = WriteManager::new(&env);

        let mut store: Store = KvvBuffer::create(&env, "kvv").unwrap();

        store.insert("key", V(1));
        assert_eq!(
            *store.scratch.get("key").unwrap(),
            hashset! {opInsert(V(1))}
        );
        store.insert("key", V(2));
        assert_eq!(
            *store.scratch.get("key").unwrap(),
            hashset! {opInsert(V(1)), opInsert(V(2))}
        );
        store.delete("key", V(1));
        assert_eq!(
            *store.scratch.get("key").unwrap(),
            hashset! {opDelete(V(1)), opInsert(V(2))}
        );
        store.insert("key", V(3));
        assert_eq!(
            *store.scratch.get("key").unwrap(),
            hashset! {opDelete(V(1)), opInsert(V(2)), opInsert(V(3))}
        );

        wm.with_writer(|mut writer| store.finalize(&mut writer))
            .unwrap();

        let store: Store = KvvBuffer::open(&env, "kvv").unwrap();
        assert_eq!(store.get(&"key").unwrap(), hashset! {V(2), V(3)});
    }

    #[test]
    fn kvv_store_get_list() {
        let arc = test_env();
        let env = arc.read().unwrap();
        let wm = WriteManager::new(&env);

        let mut store: Store = KvvBuffer::create(&env, "kvv").unwrap();

        store.insert("key", V(1));
        assert_eq!(
            *store.scratch.get("key").unwrap(),
            hashset! {opInsert(V(1))}
        );
        store.insert("key", V(2));
        assert_eq!(
            *store.scratch.get("key").unwrap(),
            hashset! {opInsert(V(1)), opInsert(V(2))}
        );
        store.delete("key", V(1));
        assert_eq!(
            *store.scratch.get("key").unwrap(),
            hashset! {opDelete(V(1)), opInsert(V(2))}
        );

        wm.with_writer(|mut writer| store.finalize(&mut writer))
            .unwrap();

        let store: Store = KvvBuffer::open(&env, "kvv").unwrap();
        assert_eq!(store.get(&"key").unwrap(), hashset! {V(2)});
    }

    #[test]
    fn kvv_store_duplicate_insert() {
        let arc = test_env();
        let env = arc.read().unwrap();

        fn add_twice(env: &Rkv) {
            let mut store: Store = KvvBuffer::create(&env, "kvv").unwrap();
            let wm = WriteManager::new(&env);

            store.insert("key", V(1));
            assert_eq!(
                *store.scratch.get("key").unwrap(),
                hashset! {opInsert(V(1))}
            );
            store.insert("key", V(1));
            assert_eq!(
                *store.scratch.get("key").unwrap(),
                hashset! {opInsert(V(1))}
            );

            wm.with_writer(|mut writer| store.finalize(&mut writer))
                .unwrap();
        }

        add_twice(&env);

        let store: Store = KvvBuffer::open(&env, "kvv").unwrap();
        assert_eq!(store.get(&"key").unwrap(), hashset! {V(1)});

        add_twice(&env);

        let store: Store = KvvBuffer::open(&env, "kvv").unwrap();
        assert_eq!(store.get(&"key").unwrap(), hashset! {V(1)});
    }

    #[test]
    fn kvv_store_duplicate_delete() {
        let arc = test_env();
        let env = arc.read().unwrap();
        let wm = WriteManager::new(&env);

        let mut store: Store = KvvBuffer::create(&env, "kvv").unwrap();
        store.insert("key", V(1));
        wm.with_writer(|mut writer| store.finalize(&mut writer))
            .unwrap();

        let mut store: Store = KvvBuffer::create(&env, "kvv").unwrap();
        store.delete("key", V(1));
        store.delete("key", V(1));
        wm.with_writer(|mut writer| store.finalize(&mut writer))
            .unwrap();

        let store: Store = KvvBuffer::open(&env, "kvv").unwrap();
        assert_eq!(store.get(&"key").unwrap(), hashset! {});
    }

    #[test]
    fn kvv_store_get_missing_key() {
        let arc = test_env();
        let env = arc.read().unwrap();
        let store: Store = KvvBuffer::create(&env, "kvv").unwrap();
        assert_eq!(store.get(&"wompwomp").unwrap(), hashset! {});
    }
}
