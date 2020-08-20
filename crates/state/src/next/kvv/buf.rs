use crate::{
    env::EnvironmentRead,
    error::{DatabaseError, DatabaseResult},
    next::{BufKey, BufMultiVal, BufferedStore},
    prelude::*,
};
use either::Either;
use rkv::MultiStore;
use std::{collections::BTreeMap, fmt::Debug};
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
    deltas: BTreeMap<V, Op>,
}

impl<V: Ord + Eq> ValuesDelta<V> {
    fn all_deleted() -> Self {
        Self {
            delete_all: true,
            deltas: BTreeMap::new(),
        }
    }
}

// This would be equivalent to the derived impl, except that this
// doesn't require `V: Default`
impl<V: Ord + Eq> Default for ValuesDelta<V> {
    fn default() -> Self {
        Self {
            delete_all: bool::default(),
            deltas: BTreeMap::new(),
        }
    }
}

/// A persisted key-value store with a transient BTreeMap to store
/// CRUD-like changes without opening a blocking read-write cursor
///
/// TODO: split the various methods for accessing data into traits,
/// and write a macro to help produce traits for every possible combination
/// of access permission, so that access can be hidden behind a limited interface
pub struct KvvBufUsed<K, V>
where
    K: BufKey,
    V: BufMultiVal,
{
    db: MultiStore,
    scratch: BTreeMap<K, ValuesDelta<V>>,
    no_dup_data: bool,
}

impl<K, V> KvvBufUsed<K, V>
where
    K: BufKey + Debug,
    V: BufMultiVal + Debug,
{
    /// Create a new KvvBufUsed from a read-only transaction and a database reference
    pub fn new(db: MultiStore) -> DatabaseResult<Self> {
        Self::new_opts(db, false)
    }

    /// Create a new KvvBufUsed from a read-only transaction and a database reference
    /// also allow switching to no_dup_data mode.
    pub fn new_opts(db: MultiStore, no_dup_data: bool) -> DatabaseResult<Self> {
        Ok(Self {
            db,
            scratch: BTreeMap::new(),
            no_dup_data,
        })
    }

    /// Get a set of values, taking the scratch space into account,
    /// or from persistence if needed
    #[instrument(skip(self, r))]
    pub fn get<'r, R: Readable>(
        &'r self,
        r: &'r R,
        k: &K,
    ) -> DatabaseResult<impl Iterator<Item = DatabaseResult<V>> + 'r> {
        // Depending on which branches get taken, this function could return
        // any of three different iterator types, in order to unify all three
        // into a single type, we return (in the happy path) a value of type
        // ```
        // Either<__GetPersistedIter, Either<__ScratchSpaceITer, Chain<...>>>
        // ```

        let values_delta = if let Some(v) = self.scratch.get(k) {
            v
        } else {
            // Only do the persisted call if it's not in the scratch
            let persisted = Self::check_not_found(self.get_persisted(r, k))?;
            trace!(?k);

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
            let persisted = Self::check_not_found(self.get_persisted(r, k))?;
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
    #[instrument(skip(self, r))]
    fn get_persisted<'r, R: Readable>(
        &self,
        r: &'r R,
        k: &K,
    ) -> DatabaseResult<impl Iterator<Item = DatabaseResult<V>> + 'r> {
        let s = trace_span!("persisted");
        let _g = s.enter();
        trace!("test");
        let iter = self.db.get(r, k)?;
        Ok(iter.filter_map(|v| match v {
            Ok((_, Some(rkv::Value::Blob(buf)))) => Some(
                holochain_serialized_bytes::decode(buf)
                    .map(|n| {
                        trace!(?n);
                        n
                    })
                    .map_err(|e| e.into()),
            ),
            Ok((_, Some(_))) => Some(Err(DatabaseError::InvalidValue)),
            Ok((_, None)) => None,
            Err(e) => Some(Err(e.into())),
        }))
    }

    fn check_not_found(
        persisted: DatabaseResult<impl Iterator<Item = DatabaseResult<V>>>,
    ) -> DatabaseResult<impl Iterator<Item = DatabaseResult<V>>> {
        let empty = std::iter::empty::<DatabaseResult<V>>();
        trace!("{:?}", line!());
        match persisted {
            Ok(persisted) => {
                trace!("{:?}", line!());
                Ok(Either::Left(persisted))
            }
            Err(DatabaseError::LmdbStoreError(err)) => match err.into_inner() {
                rkv::StoreError::LmdbError(rkv::LmdbError::NotFound) => {
                    trace!("{:?}", line!());
                    Ok(Either::Right(empty))
                }
                err => Err(err.into()),
            },
            Err(err) => Err(err),
        }
    }

    // TODO: This should be cfg test but can't because it's in a different crate
    /// Clear all scratch and db, useful for tests
    pub fn clear_all(&mut self, writer: &mut Writer) -> DatabaseResult<()> {
        self.scratch.clear();
        Ok(self.db.clear(writer)?)
    }
}

impl<K, V> BufferedStore for KvvBufUsed<K, V>
where
    K: Clone + BufKey + Debug,
    V: BufMultiVal + Debug,
{
    type Error = DatabaseError;

    fn is_clean(&self) -> bool {
        self.scratch.is_empty()
    }

    fn flush_to_txn(self, writer: &mut Writer) -> DatabaseResult<()> {
        use Op::*;
        if self.is_clean() {
            return Ok(());
        }
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
                        let buf = holochain_serialized_bytes::encode(&v)?;
                        let encoded = rkv::Value::Blob(&buf);
                        if self.no_dup_data {
                            self.db
                                .put_with_flags(
                                    writer,
                                    k.clone(),
                                    &encoded,
                                    rkv::WriteFlags::NO_DUP_DATA,
                                )
                                .or_else(|err| {
                                    // This error is a little misleading...
                                    // In a MultiStore with NO_DUP_DATA, it is
                                    // actually returned if there is a duplicate
                                    // value... which we want to ignore.
                                    if let rkv::StoreError::LmdbError(rkv::LmdbError::KeyExist) =
                                        err
                                    {
                                        Ok(())
                                    } else {
                                        Err(err)
                                    }
                                })?;
                        } else {
                            self.db.put(writer, k.clone(), &encoded)?;
                        }
                    }
                    // Skip deleting unnecessarily if we have already deleted
                    // everything
                    Delete if delete_all => {}
                    Delete => {
                        let buf = holochain_serialized_bytes::encode(&v)?;
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

    use super::{BufferedStore, KvvBufUsed, Op, ValuesDelta};
    use crate::{
        env::{ReadManager, WriteManager},
        error::{DatabaseError, DatabaseResult},
        test_utils::{test_cell_env, DbString},
        transaction::Readable,
    };
    use rkv::StoreOptions;
    use serde_derive::{Deserialize, Serialize};
    use std::collections::BTreeMap;

    #[derive(Clone, Debug, Hash, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
    struct V(pub u32);

    type Store = KvvBufUsed<DbString, V>;

    fn test_buf(
        a: &BTreeMap<DbString, ValuesDelta<V>>,
        b: impl Iterator<Item = (DbString, Vec<(V, Op)>)>,
    ) {
        for (k, v) in b {
            let val = a.get(&k).expect("Missing key");
            test_get(&val.deltas, v.into_iter());
        }
    }

    fn test_persisted<R: Readable>(r: &R, a: &Store, b: impl Iterator<Item = (DbString, Vec<V>)>) {
        for (k, v) in b {
            assert_eq!(collect_sorted(a.get_persisted(r, &k)), Ok(v));
        }
    }

    fn test_get(a: &BTreeMap<V, Op>, b: impl Iterator<Item = (V, Op)>) {
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

    #[tokio::test(threaded_scheduler)]
    async fn kvvbuf_basics() {
        let arc = test_cell_env();
        let env = arc.guard().await;

        let multi_store = env
            .inner()
            .open_multi("kvv", StoreOptions::create())
            .unwrap();

        env.with_reader::<DatabaseError, _, _>(|reader| {
            let mut store: Store = Store::new(multi_store).unwrap();
            assert_eq!(
                store
                    .get(&reader, &"key".into())
                    .unwrap()
                    .collect::<Vec<_>>(),
                []
            );

            store.delete("key".into(), V(0));
            assert_eq!(
                store
                    .get(&reader, &"key".into())
                    .unwrap()
                    .collect::<Vec<_>>(),
                []
            );

            store.insert("key".into(), V(0));
            assert_eq!(
                store
                    .get(&reader, &"key".into())
                    .unwrap()
                    .collect::<Vec<_>>(),
                [Ok(V(0))]
            );

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
            let mut store: Store = Store::new(multi_store).unwrap();
            assert_eq!(
                store
                    .get(&reader, &"key".into())
                    .unwrap()
                    .collect::<Vec<_>>(),
                [Ok(V(0))]
            );

            store.insert("key".into(), V(0));
            assert_eq!(
                store
                    .get(&reader, &"key".into())
                    .unwrap()
                    .collect::<Vec<_>>(),
                [Ok(V(0))]
            );

            store.delete("key".into(), V(0));
            assert_eq!(
                store
                    .get(&reader, &"key".into())
                    .unwrap()
                    .collect::<Vec<_>>(),
                []
            );

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
            let store: Store = Store::new(multi_store).unwrap();
            assert_eq!(
                store
                    .get(&reader, &"key".into())
                    .unwrap()
                    .collect::<Vec<_>>(),
                []
            );
            Ok(())
        })
        .unwrap();
    }

    #[tokio::test(threaded_scheduler)]
    async fn delete_all() {
        holochain_types::observability::test_run().ok();
        let arc = test_cell_env();
        let env = arc.guard().await;

        let multi_store = env
            .inner()
            .open_multi("kvv", StoreOptions::create())
            .unwrap();

        env.with_reader::<DatabaseError, _, _>(|reader| {
            let mut store: Store = Store::new(multi_store).unwrap();
            assert_eq!(
                store
                    .get(&reader, &"key".into())
                    .unwrap()
                    .collect::<Vec<_>>(),
                []
            );

            store.insert("key".into(), V(0));
            assert_eq!(
                store
                    .get(&reader, &"key".into())
                    .unwrap()
                    .collect::<Vec<_>>(),
                [Ok(V(0))]
            );

            store.insert("key".into(), V(1));
            assert_eq!(
                collect_sorted(store.get(&reader, &"key".into())),
                Ok(vec![V(0), V(1)])
            );

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
            let mut store: Store = Store::new(multi_store).unwrap();
            assert_eq!(
                collect_sorted(store.get(&reader, &"key".into())),
                Ok(vec![V(0), V(1)])
            );

            store.insert("key".into(), V(2));
            assert_eq!(
                collect_sorted(store.get(&reader, &"key".into())),
                Ok(vec![V(0), V(1), V(2)])
            );

            store.delete_all("key".into());
            assert_eq!(
                store
                    .get(&reader, &"key".into())
                    .unwrap()
                    .collect::<Vec<_>>(),
                []
            );

            store.insert("key".into(), V(3));
            assert_eq!(
                store
                    .get(&reader, &"key".into())
                    .unwrap()
                    .collect::<Vec<_>>(),
                [Ok(V(3))]
            );

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
            let store: Store = Store::new(multi_store).unwrap();
            assert_eq!(
                store
                    .get(&reader, &"key".into())
                    .unwrap()
                    .collect::<Vec<_>>(),
                [Ok(V(3))]
            );
            Ok(())
        })
        .unwrap();
    }

    /// make sure that even if there are unsorted items both
    /// before and after our idempotent operation
    /// both in the actual persistence and in our scratch
    /// that duplicates are not returned on get
    #[tokio::test(threaded_scheduler)]
    async fn idempotent_inserts() {
        let arc = test_cell_env();
        let env = arc.guard().await;

        let multi_store = env
            .inner()
            .open_multi("kvv", StoreOptions::create())
            .unwrap();

        env.with_reader::<DatabaseError, _, _>(|reader| {
            let mut store: Store = Store::new(multi_store).unwrap();
            assert_eq!(
                collect_sorted(store.get(&reader, &"key".into())),
                Ok(vec![])
            );

            store.insert("key".into(), V(2));
            assert_eq!(
                collect_sorted(store.get(&reader, &"key".into())),
                Ok(vec![V(2)])
            );

            store.insert("key".into(), V(1));
            assert_eq!(
                collect_sorted(store.get(&reader, &"key".into())),
                Ok(vec![V(1), V(2)])
            );

            store.insert("key".into(), V(1));
            assert_eq!(
                collect_sorted(store.get(&reader, &"key".into())),
                Ok(vec![V(1), V(2)])
            );

            store.insert("key".into(), V(0));
            assert_eq!(
                collect_sorted(store.get(&reader, &"key".into())),
                Ok(vec![V(0), V(1), V(2)])
            );

            env.with_commit(|mut writer| store.flush_to_txn(&mut writer))
                .unwrap();

            Ok(())
        })
        .unwrap();

        env.with_reader::<DatabaseError, _, _>(|reader| {
            let mut store: Store = Store::new(multi_store).unwrap();
            assert_eq!(
                collect_sorted(store.get(&reader, &"key".into())),
                Ok(vec![V(0), V(1), V(2)])
            );

            store.insert("key".into(), V(1));
            assert_eq!(
                collect_sorted(store.get(&reader, &"key".into())),
                Ok(vec![V(0), V(1), V(2)])
            );

            Ok(())
        })
        .unwrap();
    }

    #[tokio::test(threaded_scheduler)]
    async fn kvv_indicate_value_appends() -> DatabaseResult<()> {
        holochain_types::observability::test_run().ok();
        let arc = test_cell_env();
        let env = arc.guard().await;
        let db = env.inner().open_multi("kvv", StoreOptions::create())?;
        env.with_reader(|reader| {
            let mut buf = Store::new(db)?;

            buf.insert("a".into(), V(1));
            assert_eq!(buf.get(&reader, &"a".into())?.next().unwrap()?, V(1));
            buf.insert("a".into(), V(2));
            assert_eq!(
                collect_sorted(buf.get(&reader, &"a".into())),
                Ok(vec![V(1), V(2)])
            );
            Ok(())
        })
    }

    #[tokio::test(threaded_scheduler)]
    async fn kvv_indicate_value_overwritten() -> DatabaseResult<()> {
        holochain_types::observability::test_run().ok();
        let arc = test_cell_env();
        let env = arc.guard().await;
        let db = env.inner().open_multi("kvv", StoreOptions::create())?;
        env.with_reader(|reader| {
            let mut buf = Store::new(db)?;

            buf.insert("a".into(), V(1));
            assert_eq!(buf.get(&reader, &"a".into())?.next().unwrap()?, V(1));
            buf.delete_all("a".into());
            buf.insert("a".into(), V(2));
            assert_eq!(buf.get(&reader, &"a".into())?.next().unwrap()?, V(2));
            buf.delete("a".into(), V(2));
            buf.insert("a".into(), V(3));
            assert_eq!(buf.get(&reader, &"a".into())?.next().unwrap()?, V(3));
            Ok(())
        })
    }

    #[tokio::test(threaded_scheduler)]
    async fn kvv_deleted_persisted() -> DatabaseResult<()> {
        holochain_types::observability::test_run().ok();
        let arc = test_cell_env();
        let env = arc.guard().await;
        let db = env.inner().open_multi("kv", StoreOptions::create())?;

        {
            let mut buf = Store::new(db).unwrap();

            buf.insert("a".into(), V(1));
            buf.insert("b".into(), V(2));
            buf.insert("c".into(), V(3));

            env.with_commit(|mut writer| buf.flush_to_txn(&mut writer))?;
        }
        {
            let mut buf: KvvBufUsed<_, V> = Store::new(db).unwrap();

            buf.delete("b".into(), V(2));

            env.with_commit(|mut writer| buf.flush_to_txn(&mut writer))?;
        }
        env.with_reader(|reader| {
            let buf: KvvBufUsed<DbString, _> = Store::new(db).unwrap();
            test_persisted(
                &reader,
                &buf,
                [("a".into(), vec![V(1)]), ("c".into(), vec![V(3)])]
                    .iter()
                    .cloned(),
            );
            Ok(())
        })
    }

    #[tokio::test(threaded_scheduler)]
    async fn kvv_deleted_buffer() -> DatabaseResult<()> {
        use Op::*;
        holochain_types::observability::test_run().ok();
        let arc = test_cell_env();
        let env = arc.guard().await;
        let db = env.inner().open_multi("kv", StoreOptions::create())?;

        {
            let mut buf = Store::new(db).unwrap();

            buf.insert("a".into(), V(5));
            buf.insert("b".into(), V(4));
            buf.insert("c".into(), V(9));
            test_buf(
                &buf.scratch,
                [
                    ("a".into(), vec![(V(5), Insert)]),
                    ("b".into(), vec![(V(4), Insert)]),
                    ("c".into(), vec![(V(9), Insert)]),
                ]
                .iter()
                .cloned(),
            );
            buf.delete("b".into(), V(4));
            test_buf(
                &buf.scratch,
                [
                    ("a".into(), vec![(V(5), Insert)]),
                    ("c".into(), vec![(V(9), Insert)]),
                    ("b".into(), vec![(V(4), Delete)]),
                ]
                .iter()
                .cloned(),
            );

            env.with_commit(|mut writer| buf.flush_to_txn(&mut writer))?;
        }
        env.with_reader(|reader| {
            let buf: KvvBufUsed<DbString, _> = Store::new(db).unwrap();
            test_persisted(
                &reader,
                &buf,
                [("a".into(), vec![V(5)]), ("c".into(), vec![V(9)])]
                    .iter()
                    .cloned(),
            );
            Ok(())
        })
    }

    #[tokio::test(threaded_scheduler)]
    async fn kvv_get_buffer() -> DatabaseResult<()> {
        holochain_types::observability::test_run().ok();
        let arc = test_cell_env();
        let env = arc.guard().await;
        let db = env.inner().open_multi("kv", StoreOptions::create())?;

        env.with_reader(|reader| {
            let mut buf = Store::new(db).unwrap();

            buf.insert("a".into(), V(5));
            buf.insert("b".into(), V(4));
            buf.insert("c".into(), V(9));
            let mut n = buf.get(&reader, &"b".into())?;
            assert_eq!(n.next(), Some(Ok(V(4))));

            Ok(())
        })
    }

    #[tokio::test(threaded_scheduler)]
    async fn kvv_get_persisted() -> DatabaseResult<()> {
        holochain_types::observability::test_run().ok();
        let arc = test_cell_env();
        let env = arc.guard().await;
        let db = env.inner().open_multi("kv", StoreOptions::create())?;

        {
            let mut buf = Store::new(db).unwrap();

            buf.insert("a".into(), V(1));
            buf.insert("b".into(), V(2));
            buf.insert("c".into(), V(3));

            env.with_commit(|mut writer| buf.flush_to_txn(&mut writer))?;
        }

        env.with_reader(|reader| {
            let buf = Store::new(db).unwrap();

            let mut n = buf.get(&reader, &"b".into())?;
            assert_eq!(n.next(), Some(Ok(V(2))));
            Ok(())
        })
    }

    #[tokio::test(threaded_scheduler)]
    async fn kvv_get_del_buffer() -> DatabaseResult<()> {
        holochain_types::observability::test_run().ok();
        let arc = test_cell_env();
        let env = arc.guard().await;
        let db = env.inner().open_multi("kv", StoreOptions::create())?;

        env.with_reader(|reader| {
            let mut buf = Store::new(db).unwrap();

            buf.insert("a".into(), V(5));
            buf.insert("b".into(), V(4));
            buf.insert("c".into(), V(9));
            buf.delete("b".into(), V(4));
            let mut n = buf.get(&reader, &"b".into())?;
            assert_eq!(n.next(), None);
            Ok(())
        })
    }

    #[tokio::test(threaded_scheduler)]
    async fn kvv_get_del_persisted() -> DatabaseResult<()> {
        holochain_types::observability::test_run().ok();
        let arc = test_cell_env();
        let env = arc.guard().await;
        let db = env.inner().open_multi("kv", StoreOptions::create())?;

        {
            let mut buf = Store::new(db).unwrap();

            buf.insert("a".into(), V(1));
            buf.insert("b".into(), V(2));
            buf.insert("c".into(), V(3));

            env.with_commit(|mut writer| buf.flush_to_txn(&mut writer))?;
        }

        env.with_reader(|reader| {
            let mut buf: Store = Store::new(db).unwrap();

            buf.delete("b".into(), V(2));
            {
                let mut n = buf.get(&reader, &"b".into())?;
                assert_eq!(n.next(), None);
            }

            env.with_commit(|mut writer| buf.flush_to_txn(&mut writer))
        })?;

        env.with_reader(|reader| {
            let buf: KvvBufUsed<_, V> = Store::new(db).unwrap();

            let mut n = buf.get(&reader, &"b".into())?;
            assert_eq!(n.next(), None);
            Ok(())
        })
    }
}
