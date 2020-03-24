use super::{BufKey, BufMultiVal, BufferedStore};
use crate::{
    error::{DatabaseError, DatabaseResult},
    prelude::*,
};
use maplit::hashset;
use rkv::MultiStore;
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
pub struct KvvBuf<'env, K, V, R = Reader<'env>>
where
    K: BufKey,
    V: BufMultiVal,
    R: Readable,
{
    db: MultiStore,
    reader: &'env R,
    scratch: Scratch<K, V>,
}

impl<'env, K, V, R> KvvBuf<'env, K, V, R>
where
    K: BufKey,
    V: BufMultiVal,
    R: Readable,
{
    /// Create a new KvvBuf from a read-only transaction and a database reference
    pub fn new(reader: &'env R, db: MultiStore) -> DatabaseResult<Self> {
        Ok(Self {
            db,
            reader,
            scratch: HashMap::new(),
        })
    }

    /// Get a set of values, taking the scratch space into account,
    /// or from persistence if needed
    pub fn get(&self, k: &K) -> DatabaseResult<HashSet<V>> {
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

    /// Update the scratch space to record an Insert operation for the KV
    pub fn insert(&mut self, k: K, v: V) {
        self.scratch
            .entry(k)
            .and_modify(|ops| {
                ops.remove(&Op::Delete(Box::new(v.clone())));
                let _ = ops.insert(Op::Insert(Box::new(v.clone())));
            })
            .or_insert_with(|| hashset! { Op::Insert(Box::new(v)) });
    }

    /// Update the scratch space to record a Delete operation for the KV
    pub fn delete(&mut self, k: K, v: V) {
        self.scratch
            .entry(k)
            .and_modify(|ops| {
                ops.remove(&Op::Insert(Box::new(v.clone())));
                let _ = ops.insert(Op::Delete(Box::new(v.clone())));
            })
            .or_insert_with(|| hashset! { Op::Delete(Box::new(v)) });
    }

    /// Clear the scratch space and record a DeleteAll operation
    /// TODO: implement and make public
    fn _delete_all(&mut self, k: K) {
        if let Entry::Occupied(mut entry) = self.scratch.entry(k) {
            let _ops = entry.get_mut();
        }
        unimplemented!()
    }

    /// Fetch data from DB, deserialize into V type
    fn get_persisted(&self, k: &K) -> DatabaseResult<HashSet<V>> {
        let iter = self.db.get(self.reader, k)?;
        Ok(iter
            .map(|v| match v {
                Ok((_, Some(rkv::Value::Blob(buf)))) => Ok(Some(rmp_serde::from_read_ref(buf)?)),
                Ok((_, Some(_))) => Err(DatabaseError::InvalidValue),
                Ok((_, None)) => Ok(None),
                Err(e) => Ok(Err(e)?),
            })
            .collect::<Result<Vec<Option<V>>, DatabaseError>>()?
            .into_iter()
            .filter_map(|v| v)
            .collect())
    }
}

impl<'env, K, V, R> BufferedStore<'env> for KvvBuf<'env, K, V, R>
where
    K: Clone + BufKey,
    V: BufMultiVal,
    R: Readable,
{
    type Error = DatabaseError;

    fn flush_to_txn(self, writer: &'env mut Writer) -> DatabaseResult<()> {
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
                            if let rkv::StoreError::LmdbError(rkv::LmdbError::NotFound) = err {
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

#[cfg(test_TODO_FIX)]
pub mod tests {

    use super::{BufferedStore, KvvBuf, Op};
    use crate::test_utils::test_cell_env;
    use maplit::hashset;
    use rkv::Rkv;
    use serde_derive::{Deserialize, Serialize};

    #[derive(Clone, Debug, Hash, PartialEq, Eq, Serialize, Deserialize)]
    struct V(pub u32);

    type Store<'a> = KvvBuf<'a, &'a str, V>;

    fn op_insert<T>(v: T) -> Op<T> {
        Op::Insert(Box::new(v))
    }

    fn op_delete<T>(v: T) -> Op<T> {
        Op::Delete(Box::new(v))
    }

    #[tokio::test]
    async fn kvv_store_scratch_insert_delete() {
        let arc = test_cell_env();
        let env = arc.guard().await;
        let env = arc.env();
        let wm = WriteManager::new(&env);

        let mut store: Store = KvvBuf::create(&env, "kvv").unwrap();

        store.insert("key", V(1));
        assert_eq!(
            *store.scratch.get("key").unwrap(),
            hashset! {op_insert(V(1))}
        );
        store.insert("key", V(2));
        assert_eq!(
            *store.scratch.get("key").unwrap(),
            hashset! {op_insert(V(1)), op_insert(V(2))}
        );
        store.delete("key", V(1));
        assert_eq!(
            *store.scratch.get("key").unwrap(),
            hashset! {op_delete(V(1)), op_insert(V(2))}
        );
        store.insert("key", V(3));
        assert_eq!(
            *store.scratch.get("key").unwrap(),
            hashset! {op_delete(V(1)), op_insert(V(2)), op_insert(V(3))}
        );

        wm.with_commit(|mut writer| store.flush_to_txn(&mut writer))
            .unwrap();

        let store: Store = KvvBuf::open(&env, "kvv").unwrap();
        assert_eq!(store.get(&"key").unwrap(), hashset! {V(2), V(3)});
    }

    #[tokio::test]
    async fn kvv_store_get_list() {
        let arc = test_cell_env();
        let env = arc.guard().await;
        let env = arc.env();

        let mut store: Store = KvvBuf::create(&env, "kvv").unwrap();

        store.insert("key", V(1));
        assert_eq!(
            *store.scratch.get("key").unwrap(),
            hashset! {op_insert(V(1))}
        );
        store.insert("key", V(2));
        assert_eq!(
            *store.scratch.get("key").unwrap(),
            hashset! {op_insert(V(1)), op_insert(V(2))}
        );
        store.delete("key", V(1));
        assert_eq!(
            *store.scratch.get("key").unwrap(),
            hashset! {op_delete(V(1)), op_insert(V(2))}
        );

        wm.with_commit(|mut writer| store.flush_to_txn(&mut writer))
            .unwrap();

        let store: Store = KvvBuf::open(&env, "kvv").unwrap();
        assert_eq!(store.get(&"key").unwrap(), hashset! {V(2)});
    }

    #[tokio::test]
    async fn kvv_store_duplicate_insert() {
        let arc = test_cell_env();
        let env = arc.guard().await;
        let env = arc.env();

        fn add_twice(env: &Rkv) {
            let mut store: Store = KvvBuf::create(&env, "kvv").unwrap();
            let wm = WriteManager::new(&env);

            store.insert("key", V(1));
            assert_eq!(
                *store.scratch.get("key").unwrap(),
                hashset! {op_insert(V(1))}
            );
            store.insert("key", V(1));
            assert_eq!(
                *store.scratch.get("key").unwrap(),
                hashset! {op_insert(V(1))}
            );

            wm.with_commit(|mut writer| store.flush_to_txn(&mut writer))
                .unwrap();
        }

        add_twice(&env);

        let store: Store = KvvBuf::open(&env, "kvv").unwrap();
        assert_eq!(store.get(&"key").unwrap(), hashset! {V(1)});

        add_twice(&env);

        let store: Store = KvvBuf::open(&env, "kvv").unwrap();
        assert_eq!(store.get(&"key").unwrap(), hashset! {V(1)});
    }

    #[tokio::test]
    async fn kvv_store_duplicate_delete() {
        let arc = test_cell_env();
        let env = arc.guard().await;
        let env = arc.env();
        let wm = WriteManager::new(&env);

        let mut store: Store = KvvBuf::create(&env, "kvv").unwrap();
        store.insert("key", V(1));
        wm.with_commit(|mut writer| store.flush_to_txn(&mut writer))
            .unwrap();

        let mut store: Store = KvvBuf::create(&env, "kvv").unwrap();
        store.delete("key", V(1));
        store.delete("key", V(1));
        wm.with_commit(|mut writer| store.flush_to_txn(&mut writer))
            .unwrap();

        let store: Store = KvvBuf::open(&env, "kvv").unwrap();
        assert_eq!(store.get(&"key").unwrap(), hashset! {});
    }

    #[test]
    fn kvv_store_get_missing_key() {
        let arc = test_cell_env();
        let env = arc.guard().await;
        let env = arc.env();
        let store: Store = KvvBuf::create(&env, "kvv").unwrap();
        assert_eq!(store.get(&"wompwomp").unwrap(), hashset! {});
    }
}
