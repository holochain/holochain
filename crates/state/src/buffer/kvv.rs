use super::{BufKey, BufMultiVal, BufferedStore};
use crate::{
    error::{DatabaseError, DatabaseResult},
    prelude::*,
};
use rkv::MultiStore;
use std::{fmt::Debug, collections::HashMap};
use tracing::*;

/// Transactional operations on a KVV store
///
/// Replace is a Delete followed by an Insert
#[derive(Debug, PartialEq, Eq, Clone)]
enum Op {
    Insert,
    Delete,
}

struct ValuesDelta<V> {
    delete_all: bool,
    deltas: HashMap<V, Op>,
}

impl<V> ValuesDelta<V> {
    fn all_deleted() -> Self {
        Self {
            delete_all: true,
            deltas: HashMap::default(),
        }
    }
}

// This would be equivalent to the derived impl, except that this
// doesn't require `V: Default`
impl<V> Default for ValuesDelta<V> {
    fn default() -> Self {
        Self {
            delete_all: bool::default(),
            deltas: HashMap::default(),
        }
    }
}

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
    scratch: HashMap<K, ValuesDelta<V>>,
}

impl<'env, K, V, R> KvvBuf<'env, K, V, R>
where
    K: BufKey + Debug,
    V: BufMultiVal + Debug,
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
    pub fn get(&self, k: &K) -> DatabaseResult<impl Iterator<Item = DatabaseResult<V>> + '_> {
        // Depending on which branches get taken, this function could return
        // any of three different iterator types, in order to unify all three
        // into a single type, we return (in the happy path) a value of type
        // ```
        // Either<__GetPersistedIter, Either<__ScratchSpaceITer, Chain<...>>>
        // ```
        use either::Either;

        let values_delta = if let Some(v) = self.scratch.get(k) {
            v
        } else {
            // Only do the persisted call if it's not in the scratch
            let persisted = self.get_persisted(k)?;
            trace!(?k);
            let p: Vec<_> = persisted.collect();
            trace!(?p);
            let persisted = self.get_persisted(k)?;
            return Ok(Either::Left(persisted));
        };
        let ValuesDelta { delete_all, deltas } = values_delta;

        let from_scratch_space = deltas
            .iter()
            .filter(|(_v, op)| **op == Op::Insert)
            .map(|(v, _op)| Ok(v.clone()));

        let iter = if *delete_all {
            // If delete_all is set, return only scratch content,
            // skipping persisted content (as it will all be deleted)
            Either::Left(from_scratch_space)
        } else {
            let persisted = self.get_persisted(k)?;
            Either::Right(
                from_scratch_space
                    // Otherwise, chain it with the persisted content,
                    // skipping only things that we've specifically deleted or returned.
                    .chain(persisted.filter(move |r| match r {
                        Ok(v) => !deltas.contains_key(v),
                        Err(_e) => true,
                    })),
            )
        };

        Ok(Either::Right(iter))
    }

    /// Update the scratch space to record an Insert operation for the KV
    pub fn insert(&mut self, k: K, v: V) {
        self.scratch
            .entry(k)
            .or_default()
            .deltas
            .insert(v, Op::Insert);
    }

    /// Update the scratch space to record a Delete operation for the KV
    pub fn delete(&mut self, k: K, v: V) {
        self.scratch
            .entry(k)
            .or_default()
            .deltas
            .insert(v, Op::Delete);
    }

    /// Clear the scratch space and record a DeleteAll operation
    pub fn delete_all(&mut self, k: K) {
        self.scratch.insert(k, ValuesDelta::all_deleted());
    }

    /// Fetch data from DB, deserialize into V type
    #[instrument(skip(self))]
    fn get_persisted(&self, k: &K) -> DatabaseResult<impl Iterator<Item = DatabaseResult<V>> + '_> {
        let s = trace_span!("persisted");
        let _ = s.enter();
        trace!("test");
        let iter = self.db.get(self.reader, k)?;
        Ok(iter.filter_map(|v| match v {
            Ok((_, Some(rkv::Value::Blob(buf)))) => {
                Some(rmp_serde::from_read_ref(buf).map(|n|{trace!(?n); n}).map_err(|e| e.into()))
            }
            Ok((_, Some(_))) => Some(Err(DatabaseError::InvalidValue)),
            Ok((_, None)) => None,
            Err(e) => Some(Err(e.into())),
        }))
    }

    /*
    fn check_persisted(&self, k: &K) -> DatabaseResult<HashMap<V, Op>> {
        self.db
            .get(self.reader, k)
            .map_err(|e| e.into())
            .map(|iter| {
                iter.filter_map(|v| match v {
                    Ok((_, Some(rkv::Value::Blob(buf)))) => {
                        rmp_serde::from_read_ref(key)
                            .ok()
                            .and_then(|key| rmp_serde::from_read_ref(buf).ok().map(|v| (k, v)))
                    }
                    _ => None,
                })
                .collect()
            })
    }
    */
}

impl<'env, K, V, R> BufferedStore<'env> for KvvBuf<'env, K, V, R>
where
    K: Clone + BufKey + Debug,
    V: BufMultiVal + Debug,
    R: Readable,
{
    type Error = DatabaseError;

    fn flush_to_txn(self, writer: &'env mut Writer) -> DatabaseResult<()> {
        use Op::*;
        for (k, ValuesDelta { delete_all, deltas }) in self.scratch {
            // If delete_all is set, that we should delete everything persisted,
            // but then continue to add inserts from the ops, if present
            if delete_all {
                self.db.delete_all(writer, k.clone())?;
            }
            trace!(?k);
            trace!(?deltas);

            for (v, op) in deltas {
                match op {
                    Insert => {
                        let buf = rmp_serde::to_vec_named(&v)?;
                        let encoded = rkv::Value::Blob(&buf);
                        self.db.put(writer, k.clone(), &encoded)?;
                    }
                    // Skip deleting unnecessarily if we have already deleted
                    // everything
                    Delete if delete_all => {}
                    Delete => {
                        let buf = rmp_serde::to_vec_named(&v)?;
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
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
pub mod tests {

    use super::{BufferedStore, KvvBuf, Op, ValuesDelta};
    use crate::{
        env::{ReadManager, WriteManager},
        error::{DatabaseError, DatabaseResult},
        test_utils::test_env,
    };
    use rkv::StoreOptions;
    use serde_derive::{Deserialize, Serialize};
    use std::collections::HashMap;

    #[derive(Clone, Debug, Hash, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
    struct V(pub u32);

    type Store<'a> = KvvBuf<'a, &'a str, V>;

    fn test_buf(
        a: &HashMap<&str, ValuesDelta<V>>,
        b: impl Iterator<Item = (&'static str, Vec<(V, Op)>)>,
    ) {
        for (k, v) in b {
            let val = a.get(&k).expect("Missing key");
            test_get(&val.deltas, v.into_iter());
        }
    }

    fn test_persisted(a: &KvvBuf<&str, V>, b: impl Iterator<Item = (&'static str, Vec<V>)>) {
        for (k, v) in b {
            assert_eq!(collect_sorted(a.get_persisted(&k)), Ok(v));
        }
    }

    fn test_get(a: &HashMap<V, Op>, b: impl Iterator<Item = (V, Op)>) {
        for (k, v) in b {
            let val = a.get(&k).expect("Missing key");
            assert_eq!(*val, v);
        }
    }

    fn collect_sorted<T: Ord, E, I: IntoIterator<Item = Result<T, E>>>(
        iter: Result<I, E>,
    ) -> Result<Vec<T>, E> {
        let mut vec = iter?.into_iter().collect::<Result<Vec<_>, _>>()?;
        vec.sort_unstable();
        Ok(vec)
    }

    #[tokio::test]
    async fn kvvbuf_basics() {
        let arc = test_env();
        let env = arc.guard().await;

        let multi_store = env
            .inner()
            .open_multi("kvv", StoreOptions::create())
            .unwrap();

        env.with_reader::<DatabaseError, _, _>(|reader| {
            let mut store: Store = KvvBuf::new(&reader, multi_store).unwrap();
            assert_eq!(store.get(&"key").unwrap().collect::<Vec<_>>(), []);

            store.delete("key", V(0));
            assert_eq!(store.get(&"key").unwrap().collect::<Vec<_>>(), []);

            store.insert("key", V(0));
            assert_eq!(store.get(&"key").unwrap().collect::<Vec<_>>(), [Ok(V(0))]);

            env.with_commit(|mut writer| store.flush_to_txn(&mut writer))
                .unwrap();

            Ok(())
        })
        .unwrap();

        let multi_store = env
            .inner()
            .open_multi("kvv", StoreOptions::default())
            .unwrap();

        env.with_reader::<DatabaseError, _, _>(|reader| {
            let mut store: Store = KvvBuf::new(&reader, multi_store).unwrap();
            assert_eq!(store.get(&"key").unwrap().collect::<Vec<_>>(), [Ok(V(0))]);

            store.insert("key", V(0));
            assert_eq!(store.get(&"key").unwrap().collect::<Vec<_>>(), [Ok(V(0))]);

            store.delete("key", V(0));
            assert_eq!(store.get(&"key").unwrap().collect::<Vec<_>>(), []);

            env.with_commit(|mut writer| store.flush_to_txn(&mut writer))
                .unwrap();

            Ok(())
        })
        .unwrap();

        let multi_store = env
            .inner()
            .open_multi("kvv", StoreOptions::default())
            .unwrap();

        env.with_reader::<DatabaseError, _, _>(|reader| {
            let store: Store = KvvBuf::new(&reader, multi_store).unwrap();
            assert_eq!(store.get(&"key").unwrap().collect::<Vec<_>>(), []);
            Ok(())
        })
        .unwrap();
    }

    #[tokio::test]
    async fn delete_all() {
        let arc = test_env();
        let env = arc.guard().await;

        let multi_store = env
            .inner()
            .open_multi("kvv", StoreOptions::create())
            .unwrap();

        env.with_reader::<DatabaseError, _, _>(|reader| {
            let mut store: Store = KvvBuf::new(&reader, multi_store).unwrap();
            assert_eq!(store.get(&"key").unwrap().collect::<Vec<_>>(), []);

            store.insert("key", V(0));
            assert_eq!(store.get(&"key").unwrap().collect::<Vec<_>>(), [Ok(V(0))]);

            store.insert("key", V(1));
            assert_eq!(collect_sorted(store.get(&"key")), Ok(vec![V(0), V(1)]));

            env.with_commit(|mut writer| store.flush_to_txn(&mut writer))
                .unwrap();

            Ok(())
        })
        .unwrap();

        let multi_store = env
            .inner()
            .open_multi("kvv", StoreOptions::default())
            .unwrap();

        env.with_reader::<DatabaseError, _, _>(|reader| {
            let mut store: Store = KvvBuf::new(&reader, multi_store).unwrap();
            assert_eq!(collect_sorted(store.get(&"key")), Ok(vec![V(0), V(1)]));

            store.insert("key", V(2));
            assert_eq!(
                collect_sorted(store.get(&"key")),
                Ok(vec![V(0), V(1), V(2)])
            );

            store.delete_all("key");
            assert_eq!(store.get(&"key").unwrap().collect::<Vec<_>>(), []);

            store.insert("key", V(3));
            assert_eq!(store.get(&"key").unwrap().collect::<Vec<_>>(), [Ok(V(3))]);

            env.with_commit(|mut writer| store.flush_to_txn(&mut writer))
                .unwrap();

            Ok(())
        })
        .unwrap();

        let multi_store = env
            .inner()
            .open_multi("kvv", StoreOptions::default())
            .unwrap();

        env.with_reader::<DatabaseError, _, _>(|reader| {
            let store: Store = KvvBuf::new(&reader, multi_store).unwrap();
            assert_eq!(store.get(&"key").unwrap().collect::<Vec<_>>(), [Ok(V(3))]);
            Ok(())
        })
        .unwrap();
    }

    #[tokio::test]
    async fn idempotent_inserts() {
        let arc = test_env();
        let env = arc.guard().await;

        let multi_store = env
            .inner()
            .open_multi("kvv", StoreOptions::create())
            .unwrap();

        env.with_reader::<DatabaseError, _, _>(|reader| {
            let mut store: Store = KvvBuf::new(&reader, multi_store).unwrap();
            assert_eq!(store.get(&"key").unwrap().collect::<Vec<_>>(), []);

            store.insert("key", V(0));
            assert_eq!(store.get(&"key").unwrap().collect::<Vec<_>>(), [Ok(V(0))]);

            store.insert("key", V(0));
            assert_eq!(store.get(&"key").unwrap().collect::<Vec<_>>(), [Ok(V(0))]);

            env.with_commit(|mut writer| store.flush_to_txn(&mut writer))
                .unwrap();

            Ok(())
        })
        .unwrap();

        env.with_reader::<DatabaseError, _, _>(|reader| {
            let mut store: Store = KvvBuf::new(&reader, multi_store).unwrap();
            assert_eq!(store.get(&"key").unwrap().collect::<Vec<_>>(), [Ok(V(0))]);

            store.insert("key", V(0));
            assert_eq!(store.get(&"key").unwrap().collect::<Vec<_>>(), [Ok(V(0))]);

            Ok(())
        })
        .unwrap();
    }

    #[tokio::test]
    async fn kvv_indicate_value_appends() -> DatabaseResult<()> {
        sx_types::observability::test_run().ok();
        let arc = test_env();
        let env = arc.guard().await;
        let db = env.inner().open_multi("kvv", StoreOptions::create())?;
        env.with_reader(|reader| {
            let mut buf = KvvBuf::new(&reader, db)?;

            buf.insert("a", V(1));
            assert_eq!(buf.get(&"a")?.next().unwrap()?, V(1));
            buf.insert("a", V(2));
            assert_eq!(collect_sorted(buf.get(&"a")), Ok(vec![V(1), V(2)]));
            Ok(())
        })
    }

    #[tokio::test]
    async fn kvv_indicate_value_overwritten() -> DatabaseResult<()> {
        sx_types::observability::test_run().ok();
        let arc = test_env();
        let env = arc.guard().await;
        let db = env.inner().open_multi("kvv", StoreOptions::create())?;
        env.with_reader(|reader| {
            let mut buf = KvvBuf::new(&reader, db)?;

            buf.insert("a", V(1));
            assert_eq!(buf.get(&"a")?.next().unwrap()?, V(1));
            buf.delete_all("a");
            buf.insert("a", V(2));
            assert_eq!(buf.get(&"a")?.next().unwrap()?, V(2));
            buf.delete("a", V(2));
            buf.insert("a", V(3));
            assert_eq!(buf.get(&"a")?.next().unwrap()?, V(3));
            Ok(())
        })
    }

    #[tokio::test]
    async fn kvv_deleted_persisted() -> DatabaseResult<()> {
        sx_types::observability::test_run().ok();
        let arc = test_env();
        let env = arc.guard().await;
        let db = env.inner().open_multi("kv", StoreOptions::create())?;

        env.with_reader(|reader| {
            let mut buf = KvvBuf::new(&reader, db).unwrap();

            buf.insert("a", V(1));
            buf.insert("b", V(2));
            buf.insert("c", V(3));

            env.with_commit(|mut writer| buf.flush_to_txn(&mut writer))
        })?;
        env.with_reader(|reader| {
            let mut buf: KvvBuf<_, V> = KvvBuf::new(&reader, db).unwrap();

            buf.delete("b", V(2));

            env.with_commit(|mut writer| buf.flush_to_txn(&mut writer))
        })?;
        env.with_reader(|reader| {
            let buf: KvvBuf<&str, _> = KvvBuf::new(&reader, db).unwrap();
            test_persisted(&buf, [("a", vec![V(1)]), ("c", vec![V(3)])].iter().cloned());
            Ok(())
        })
    }

    #[tokio::test]
    async fn kvv_deleted_buffer() -> DatabaseResult<()> {
        use Op::*;
        sx_types::observability::test_run().ok();
        let arc = test_env();
        let env = arc.guard().await;
        let db = env.inner().open_multi("kv", StoreOptions::create())?;

        env.with_reader(|reader| {
            let mut buf = KvvBuf::new(&reader, db).unwrap();

            buf.insert("a", V(5));
            buf.insert("b", V(4));
            buf.insert("c", V(9));
            test_buf(
                &buf.scratch,
                [
                    ("a", vec![(V(5), Insert)]),
                    ("b", vec![(V(4), Insert)]),
                    ("c", vec![(V(9), Insert)]),
                ]
                .iter()
                .cloned(),
            );
            buf.delete("b", V(4));
            test_buf(
                &buf.scratch,
                [
                    ("a", vec![(V(5), Insert)]),
                    ("c", vec![(V(9), Insert)]),
                    ("b", vec![(V(4), Delete)]),
                ]
                .iter()
                .cloned(),
            );

            env.with_commit(|mut writer| buf.flush_to_txn(&mut writer))
        })?;
        env.with_reader(|reader| {
            let buf: KvvBuf<&str, _> = KvvBuf::new(&reader, db).unwrap();
            test_persisted(&buf, [("a", vec![V(5)]), ("c", vec![V(9)])].iter().cloned());
            Ok(())
        })
    }

    #[tokio::test]
    async fn kvv_get_buffer() -> DatabaseResult<()> {
        sx_types::observability::test_run().ok();
        let arc = test_env();
        let env = arc.guard().await;
        let db = env.inner().open_multi("kv", StoreOptions::create())?;

        env.with_reader(|reader| {
            let mut buf = KvvBuf::new(&reader, db).unwrap();

            buf.insert("a", V(5));
            buf.insert("b", V(4));
            buf.insert("c", V(9));
            let mut n = buf.get(&"b")?;
            assert_eq!(n.next(), Some(Ok(V(4))));

            Ok(())
        })
    }

    #[tokio::test]
    async fn kvv_get_persisted() -> DatabaseResult<()> {
        sx_types::observability::test_run().ok();
        let arc = test_env();
        let env = arc.guard().await;
        let db = env.inner().open_multi("kv", StoreOptions::create())?;

        env.with_reader(|reader| {
            let mut buf = KvvBuf::new(&reader, db).unwrap();

            buf.insert("a", V(1));
            buf.insert("b", V(2));
            buf.insert("c", V(3));

            env.with_commit(|mut writer| buf.flush_to_txn(&mut writer))
        })?;

        env.with_reader(|reader| {
            let buf = KvvBuf::new(&reader, db).unwrap();

            let mut n = buf.get(&"b")?;
            assert_eq!(n.next(), Some(Ok(V(2))));
            Ok(())
        })
    }

    #[tokio::test]
    async fn kvv_get_del_buffer() -> DatabaseResult<()> {
        sx_types::observability::test_run().ok();
        let arc = test_env();
        let env = arc.guard().await;
        let db = env.inner().open_multi("kv", StoreOptions::create())?;

        env.with_reader(|reader| {
            let mut buf = KvvBuf::new(&reader, db).unwrap();

            buf.insert("a", V(5));
            buf.insert("b", V(4));
            buf.insert("c", V(9));
            buf.delete("b", V(4));
            let mut n = buf.get(&"b")?;
            assert_eq!(n.next(), None);
            Ok(())
        })
    }

    #[tokio::test]
    async fn kvv_get_del_persisted() -> DatabaseResult<()> {
        sx_types::observability::test_run().ok();
        let arc = test_env();
        let env = arc.guard().await;
        let db = env.inner().open_multi("kv", StoreOptions::create())?;

        env.with_reader(|reader| {
            let mut buf = KvvBuf::new(&reader, db).unwrap();

            buf.insert("a", V(1));
            buf.insert("b", V(2));
            buf.insert("c", V(3));

            env.with_commit(|mut writer| buf.flush_to_txn(&mut writer))
        })?;

        env.with_reader(|reader| {
            let mut buf: KvvBuf<&str, V> = KvvBuf::new(&reader, db).unwrap();

            buf.delete("b", V(2));
            {
                let mut n = buf.get(&"b")?;
                assert_eq!(n.next(), None);
            }

            env.with_commit(|mut writer| buf.flush_to_txn(&mut writer))
        })?;

        env.with_reader(|reader| {
            let buf: KvvBuf<_, V> = KvvBuf::new(&reader, db).unwrap();

            let mut n = buf.get(&"b")?;
            assert_eq!(n.next(), Some(Ok(V(1))));
            assert_eq!(n.next(), Some(Ok(V(3))));

            let mut n = buf.get(&"b")?;
            assert_eq!(n.next(), None);
            Ok(())
        })
    }
}
