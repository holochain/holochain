//! An interface to an LMDB key-value store, with integer keys
//! This is unfortunately pure copy past from KvBuf, since Rust doesn't support specialization yet
//! TODO, find *some* way to DRY up the two

use super::{BufIntKey, BufVal, BufferedStore};
use crate::{
    error::{DatabaseError, DatabaseResult},
    prelude::{Readable, Reader, Writer},
};
use rkv::IntegerStore;

use std::collections::HashMap;
use tracing::*;

/// Transactional operations on a KV store with integer keys
/// Put: add or replace this KV
/// Delete: remove the KV
#[derive(Clone, Debug, PartialEq)]
enum Op<V> {
    Put(Box<V>),
    Delete,
}

/// A persisted key-value store with a transient HashMap to store
/// CRUD-like changes without opening a blocking read-write cursor
///
/// TODO: split the various methods for accessing data into traits,
/// and write a macro to help produce traits for every possible combination
/// of access permission, so that access can be hidden behind a limited interface
pub struct IntKvBuf<'env, K, V, R = Reader<'env>>
where
    K: BufIntKey,
    V: BufVal,
    R: Readable,
{
    db: IntegerStore<K>,
    reader: &'env R,
    scratch: HashMap<K, Op<V>>,
}

impl<'env, K, V, R> IntKvBuf<'env, K, V, R>
where
    K: BufIntKey,
    V: BufVal,
    R: Readable,
{
    /// Create a new IntKvBuf from a read-only transaction and a database reference
    pub fn new(reader: &'env R, db: IntegerStore<K>) -> DatabaseResult<Self> {
        Ok(Self {
            db,
            reader,
            scratch: HashMap::new(),
        })
    }

    /// Create a new IntKvBuf from a new read-only transaction, using the same database
    /// as an existing IntKvBuf. Useful for getting a fresh read-only snapshot of a database.
    pub fn with_reader<RR: Readable>(&self, reader: &'env RR) -> IntKvBuf<'env, K, V, RR> {
        IntKvBuf {
            db: self.db,
            reader,
            scratch: HashMap::new(),
        }
    }

    /// Get a value, taking the scratch space into account,
    /// or from persistence if needed
    pub fn get(&self, k: K) -> DatabaseResult<Option<V>> {
        use Op::*;
        let val = match self.scratch.get(&k) {
            Some(Put(scratch_val)) => Some(*scratch_val.clone()),
            Some(Delete) => None,
            None => self.get_persisted(k)?,
        };
        Ok(val)
    }

    /// Adds a Put [Op::Put](Op) to the scratch that will be run on commit.
    pub fn put(&mut self, k: K, v: V) {
        self.scratch.insert(k, Op::Put(Box::new(v)));
    }

    /// Adds a [Op::Delete](Op) to the scratch space that will be run on commit
    pub fn delete(&mut self, k: K) {
        self.scratch.insert(k, Op::Delete);
    }

    /// Fetch data from DB, deserialize into V type
    fn get_persisted(&self, k: K) -> DatabaseResult<Option<V>> {
        match self.db.get(self.reader, k)? {
            Some(rkv::Value::Blob(buf)) => Ok(Some(rmp_serde::from_read_ref(buf)?)),
            None => Ok(None),
            Some(_) => Err(DatabaseError::InvalidValue),
        }
    }

    /// Iterate over the underlying persisted data, NOT taking the scratch space into consideration
    pub fn iter_raw(&self) -> DatabaseResult<SingleIntIter<K, V>> {
        Ok(SingleIntIter::new(self.db.iter_start(self.reader)?))
    }

    /// Iterate over the underlying persisted data in reverse, NOT taking the scratch space into consideration
    pub fn iter_raw_reverse(&self) -> DatabaseResult<SingleIntIter<K, V>> {
        Ok(SingleIntIter::new(self.db.iter_end(self.reader)?))
    }
}

impl<'env, K, V, R> BufferedStore<'env> for IntKvBuf<'env, K, V, R>
where
    K: BufIntKey,
    V: BufVal,
    R: Readable,
{
    type Error = DatabaseError;

    fn flush_to_txn(self, writer: &'env mut Writer) -> DatabaseResult<()> {
        use Op::*;
        for (k, op) in self.scratch.iter() {
            match op {
                Put(v) => {
                    let buf = rmp_serde::to_vec_named(v)?;
                    let encoded = rkv::Value::Blob(&buf);
                    self.db.put(writer, *k, &encoded)?;
                }
                Delete => match self.db.delete(writer, *k) {
                    Err(rkv::StoreError::LmdbError(rkv::LmdbError::NotFound)) => (),
                    r => r?,
                },
            }
        }
        Ok(())
    }
}

pub struct SingleIntIter<'env, K, V>(
    rkv::store::single::Iter<'env>,
    std::marker::PhantomData<(K, V)>,
);

impl<'env, K, V> SingleIntIter<'env, K, V> {
    pub fn new(iter: rkv::store::single::Iter<'env>) -> Self {
        Self(iter, std::marker::PhantomData)
    }
}

/// Iterator over key, value pairs. Both keys and values are deserialized
/// to their proper types.
/// TODO: Use FallibleIterator to prevent panics within iteration
impl<'env, K, V> Iterator for SingleIntIter<'env, K, V>
where
    K: BufIntKey,
    V: BufVal,
{
    type Item = (K, V);

    fn next(&mut self) -> Option<Self::Item> {
        match self.0.next() {
            Some(Ok((k, Some(rkv::Value::Blob(buf))))) => Some((
                K::from_bytes(k).expect("Failed to deserialize key"),
                rmp_serde::from_read_ref(buf).expect("Failed to deserialize value"),
            )),
            None => None,
            x => {
                error!(?x);
                panic!("TODO");
            }
        }
    }
}

#[cfg(test)]
pub mod tests {

    use super::{BufferedStore, IntKvBuf, *};
    use crate::{
        env::{ReadManager, WriteManager},
        error::DatabaseResult,
        test_utils::test_cell_env,
    };
    use rkv::StoreOptions;
    use serde_derive::{Deserialize, Serialize};

    #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
    struct TestVal {
        name: String,
    }

    #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
    struct V(u32);

    type Store<'a> = IntKvBuf<'a, u32, V>;

    macro_rules! res {
        ($key:expr, $op:ident, $val:expr) => {
            ($key, Op::$op(Box::new(V($val))))
        };
        ($key:expr, $op:ident) => {
            ($key, Op::$op)
        };
    }

    fn test_buf(a: &HashMap<u32, Op<V>>, b: impl Iterator<Item = (u32, Op<V>)>) {
        for (k, v) in b {
            let val = a.get(&k).expect("Missing key");
            assert_eq!(*val, v);
        }
    }

    #[tokio::test]
    async fn kvint_iterators() -> DatabaseResult<()> {
        let arc = test_cell_env();
        let env = arc.guard().await;
        let db = env.inner().open_integer("kv", StoreOptions::create())?;

        env.with_reader(|reader| {
            let mut buf: Store = IntKvBuf::new(&reader, db)?;

            buf.put(1, V(1));
            buf.put(2, V(2));
            buf.put(3, V(3));
            buf.put(4, V(4));
            buf.put(5, V(5));

            env.with_commit(|mut writer| buf.flush_to_txn(&mut writer))
        })?;

        env.with_reader(|reader| {
            let buf: Store = IntKvBuf::new(&reader, db)?;

            let forward: Vec<_> = buf.iter_raw()?.collect();
            let reverse: Vec<_> = buf.iter_raw_reverse()?.collect();

            assert_eq!(
                forward,
                vec![(1, V(1)), (2, V(2)), (3, V(3)), (4, V(4)), (5, V(5))]
            );
            assert_eq!(
                reverse,
                vec![(5, V(5)), (4, V(4)), (3, V(3)), (2, V(2)), (1, V(1))]
            );
            Ok(())
        })
    }

    #[tokio::test]
    async fn kvint_empty_iterators() -> DatabaseResult<()> {
        let arc = test_cell_env();
        let env = arc.guard().await;
        let db = env.inner().open_integer("kv", StoreOptions::create())?;

        env.with_reader(|reader| {
            let buf: Store = IntKvBuf::new(&reader, db).unwrap();

            let forward: Vec<_> = buf.iter_raw().unwrap().collect();
            let reverse: Vec<_> = buf.iter_raw_reverse().unwrap().collect();

            assert_eq!(forward, vec![]);
            assert_eq!(reverse, vec![]);
            Ok(())
        })
    }
    #[tokio::test]
    async fn kvint_indicate_value_overwritten() -> DatabaseResult<()> {
        sx_types::observability::test_run().ok();
        let arc = test_cell_env();
        let env = arc.guard().await;
        let db = env.inner().open_integer("kv", StoreOptions::create())?;
        env.with_reader(|reader| {
            let mut buf: Store = IntKvBuf::new(&reader, db)?;

            buf.put(1, V(1));
            assert_eq!(Some(V(1)), buf.get(1)?);
            buf.put(1, V(2));
            assert_eq!(Some(V(2)), buf.get(1)?);
            Ok(())
        })
    }

    #[tokio::test]
    async fn kvint_deleted_persisted() -> DatabaseResult<()> {
        use tracing::*;
        sx_types::observability::test_run().ok();
        let arc = test_cell_env();
        let env = arc.guard().await;
        let db = env.inner().open_integer("kv", StoreOptions::create())?;

        env.with_reader(|reader| {
            let mut buf: Store = IntKvBuf::new(&reader, db).unwrap();

            buf.put(1, V(1));
            buf.put(2, V(2));
            buf.put(3, V(3));

            env.with_commit(|mut writer| buf.flush_to_txn(&mut writer))
        })?;
        env.with_reader(|reader| {
            let mut buf: Store = IntKvBuf::new(&reader, db).unwrap();

            buf.delete(2);

            env.with_commit(|mut writer| buf.flush_to_txn(&mut writer))
        })?;
        env.with_reader(|reader| {
            let buf: Store = IntKvBuf::new(&reader, db).unwrap();

            let forward: Vec<_> = buf.iter_raw().unwrap().collect();
            debug!(?forward);
            assert_eq!(forward, vec![(1, V(1)), (3, V(3))]);
            Ok(())
        })
    }

    #[tokio::test]
    async fn kvint_deleted_buffer() -> DatabaseResult<()> {
        sx_types::observability::test_run().ok();
        let arc = test_cell_env();
        let env = arc.guard().await;
        let db = env.inner().open_integer("kv", StoreOptions::create())?;

        env.with_reader(|reader| {
            let mut buf: Store = IntKvBuf::new(&reader, db).unwrap();

            buf.put(1, V(5));
            buf.put(2, V(4));
            buf.put(3, V(9));
            test_buf(
                &buf.scratch,
                [res!(1, Put, 5), res!(2, Put, 4), res!(3, Put, 9)]
                    .iter()
                    .cloned(),
            );
            buf.delete(2);
            test_buf(
                &buf.scratch,
                [res!(1, Put, 5), res!(3, Put, 9), res!(2, Delete)]
                    .iter()
                    .cloned(),
            );

            env.with_commit(|mut writer| buf.flush_to_txn(&mut writer))
        })?;
        env.with_reader(|reader| {
            let buf: Store = IntKvBuf::new(&reader, db).unwrap();

            let forward: Vec<_> = buf.iter_raw().unwrap().collect();
            assert_eq!(forward, vec![(1, V(5)), (3, V(9))]);
            Ok(())
        })
    }

    #[tokio::test]
    async fn kvint_get_buffer() -> DatabaseResult<()> {
        sx_types::observability::test_run().ok();
        let arc = test_cell_env();
        let env = arc.guard().await;
        let db = env.inner().open_integer("kv", StoreOptions::create())?;

        env.with_reader(|reader| {
            let mut buf: Store = IntKvBuf::new(&reader, db).unwrap();

            buf.put(1, V(5));
            buf.put(2, V(4));
            buf.put(3, V(9));
            let n = buf.get(2)?;
            assert_eq!(n, Some(V(4)));

            Ok(())
        })
    }

    #[tokio::test]
    async fn kvint_get_persisted() -> DatabaseResult<()> {
        sx_types::observability::test_run().ok();
        let arc = test_cell_env();
        let env = arc.guard().await;
        let db = env.inner().open_integer("kv", StoreOptions::create())?;

        env.with_reader(|reader| {
            let mut buf: Store = IntKvBuf::new(&reader, db).unwrap();

            buf.put(1, V(1));
            buf.put(2, V(2));
            buf.put(3, V(3));

            env.with_commit(|mut writer| buf.flush_to_txn(&mut writer))
        })?;

        env.with_reader(|reader| {
            let buf: Store = IntKvBuf::new(&reader, db).unwrap();

            let n = buf.get(2)?;
            assert_eq!(n, Some(V(2)));
            Ok(())
        })
    }

    #[tokio::test]
    async fn kvint_get_del_buffer() -> DatabaseResult<()> {
        sx_types::observability::test_run().ok();
        let arc = test_cell_env();
        let env = arc.guard().await;
        let db = env.inner().open_integer("kv", StoreOptions::create())?;

        env.with_reader(|reader| {
            let mut buf: Store = IntKvBuf::new(&reader, db).unwrap();

            buf.put(1, V(5));
            buf.put(2, V(4));
            buf.put(3, V(9));
            buf.delete(2);
            let n = buf.get(2)?;
            assert_eq!(n, None);
            Ok(())
        })
    }

    #[tokio::test]
    async fn kvint_get_del_persisted() -> DatabaseResult<()> {
        sx_types::observability::test_run().ok();
        let arc = test_cell_env();
        let env = arc.guard().await;
        let db = env.inner().open_integer("kv", StoreOptions::create())?;

        env.with_reader(|reader| {
            let mut buf: Store = IntKvBuf::new(&reader, db).unwrap();

            buf.put(1, V(1));
            buf.put(2, V(2));
            buf.put(3, V(3));

            env.with_commit(|mut writer| buf.flush_to_txn(&mut writer))
        })?;

        env.with_reader(|reader| {
            let mut buf: Store = IntKvBuf::new(&reader, db).unwrap();

            buf.delete(2);
            let n = buf.get(2)?;
            assert_eq!(n, None);

            env.with_commit(|mut writer| buf.flush_to_txn(&mut writer))
        })?;

        env.with_reader(|reader| {
            let buf: Store = IntKvBuf::new(&reader, db).unwrap();

            let n = buf.get(2)?;
            assert_eq!(n, None);
            Ok(())
        })
    }
}
