use super::{BufKey, BufVal, BufferedStore};
use crate::{
    error::{DatabaseError, DatabaseResult},
    prelude::{Readable, Reader, Writer},
};
use rkv::SingleStore;

use std::collections::HashMap;
use tracing::*;

/// Transactional operations on a KV store
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
pub struct KvBuf<'env, K, V, R = Reader<'env>>
where
    K: BufKey,
    V: BufVal,
{
    db: SingleStore,
    reader: &'env R,
    scratch: HashMap<K, Op<V>>,
}

impl<'env, K, V, R> KvBuf<'env, K, V, R>
where
    K: BufKey,
    V: BufVal,
    R: Readable,
{
    /// Create a new KvBuf from a read-only transaction and a database reference
    pub fn new(reader: &'env R, db: SingleStore) -> DatabaseResult<Self> {
        Ok(Self {
            db,
            reader,
            scratch: HashMap::new(),
        })
    }

    /// Get a value, taking the scratch space into account,
    /// or from persistence if needed
    pub fn get(&self, k: &K) -> DatabaseResult<Option<V>> {
        use Op::*;
        let val = match self.scratch.get(k) {
            Some(Put(scratch_val)) => Some(*scratch_val.clone()),
            Some(Delete) => None,
            None => self.get_persisted(k)?,
        };
        Ok(val)
    }

    /// Update the scratch space to record a Put operation for the KV
    pub fn put(&mut self, k: K, v: V) {
        self.scratch.insert(k, Op::Put(Box::new(v)));
    }

    /// Update the scratch space to record a Delete operation for the KV
    pub fn delete(&mut self, k: K) {
        self.scratch.insert(k, Op::Delete);
    }

    /// Fetch data from DB, deserialize into V type
    fn get_persisted(&self, k: &K) -> DatabaseResult<Option<V>> {
        match self.db.get(self.reader, k)? {
            Some(rkv::Value::Blob(buf)) => Ok(Some(rmp_serde::from_read_ref(buf)?)),
            None => Ok(None),
            Some(_) => Err(DatabaseError::InvalidValue),
        }
    }

    /// Iterate over the underlying persisted data, NOT taking the scratch space into consideration
    pub fn iter_raw(&self) -> DatabaseResult<SingleIter<V>> {
        Ok(SingleIter::new(self.db.iter_start(self.reader)?))
    }

    /// Iterate over the underlying persisted data in reverse, NOT taking the scratch space into consideration
    pub fn iter_raw_reverse(&self) -> DatabaseResult<SingleIter<V>> {
        Ok(SingleIter::new(self.db.iter_end(self.reader)?))
    }
}

impl<'env, K, V, R> BufferedStore<'env> for KvBuf<'env, K, V, R>
where
    K: BufKey,
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
                    self.db.put(writer, k, &encoded)?;
                }
                Delete => match self.db.delete(writer, k) {
                    Err(rkv::StoreError::LmdbError(rkv::LmdbError::NotFound)) => (),
                    r => r?,
                },
            }
        }
        Ok(())
    }
}

pub struct SingleIter<'env, V>(rkv::store::single::Iter<'env>, std::marker::PhantomData<V>);

impl<'env, V> SingleIter<'env, V> {
    pub fn new(iter: rkv::store::single::Iter<'env>) -> Self {
        Self(iter, std::marker::PhantomData)
    }
}

/// Iterate over key, value pairs in this store using low-level LMDB iterators
/// NOTE: While the value is deserialized to the proper type, the key is returned as raw bytes.
/// This is to enable a wider range of keys, such as String, because there is no uniform trait which
/// enables conversion from a byte slice to a given type.
/// FIXME: Use FallibleIterator to prevent panics within iteration
impl<'env, V> Iterator for SingleIter<'env, V>
where
    V: BufVal,
{
    type Item = (&'env [u8], V);

    fn next(&mut self) -> Option<Self::Item> {
        match self.0.next() {
            Some(Ok((k, Some(rkv::Value::Blob(buf))))) => Some((
                k,
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

    use super::{BufferedStore, KvBuf, Op};
    use crate::{
        env::{ReadManager, WriteManager},
        error::{DatabaseError, DatabaseResult},
        test_utils::test_cell_env,
    };
    use rkv::StoreOptions;
    use serde_derive::{Deserialize, Serialize};
    use std::collections::HashMap;

    #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
    struct TestVal {
        name: String,
    }

    #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
    struct V(u32);

    type TestBuf<'a> = KvBuf<'a, &'a str, V>;

    macro_rules! res {
        ($key:expr, $op:ident, $val:expr) => {
            ($key, Op::$op(Box::new(V($val))))
        };
        ($key:expr, $op:ident) => {
            ($key, Op::$op)
        };
    }

    fn test_buf(a: &HashMap<&str, Op<V>>, b: impl Iterator<Item = (&'static str, Op<V>)>) {
        for (k, v) in b {
            let val = a.get(&k).expect("Missing key");
            assert_eq!(*val, v);
        }
    }

    #[tokio::test(threaded_scheduler)]
    async fn kv_iterators() -> DatabaseResult<()> {
        let arc = test_cell_env().await;
        let env = arc.guard().await;
        let db = env.inner().open_single("kv", StoreOptions::create())?;

        env.with_reader::<DatabaseError, _, _>(|reader| {
            let mut buf: TestBuf = KvBuf::new(&reader, db)?;

            buf.put("a", V(1));
            buf.put("b", V(2));
            buf.put("c", V(3));
            buf.put("d", V(4));
            buf.put("e", V(5));

            env.with_commit(|mut writer| buf.flush_to_txn(&mut writer))?;
            Ok(())
        })?;

        env.with_reader(|reader| {
            let buf: TestBuf = KvBuf::new(&reader, db)?;

            let forward: Vec<_> = buf.iter_raw()?.map(|(_, v)| v).collect();
            let reverse: Vec<_> = buf.iter_raw_reverse()?.map(|(_, v)| v).collect();

            assert_eq!(forward, vec![V(1), V(2), V(3), V(4), V(5)]);
            assert_eq!(reverse, vec![V(5), V(4), V(3), V(2), V(1)]);
            Ok(())
        })
    }

    #[tokio::test(threaded_scheduler)]
    async fn kv_empty_iterators() -> DatabaseResult<()> {
        let arc = test_cell_env().await;
        let env = arc.guard().await;
        let db = env
            .inner()
            .open_single("kv", StoreOptions::create())
            .unwrap();

        env.with_reader(|reader| {
            let buf: TestBuf = KvBuf::new(&reader, db).unwrap();

            let forward: Vec<_> = buf.iter_raw().unwrap().collect();
            let reverse: Vec<_> = buf.iter_raw_reverse().unwrap().collect();

            assert_eq!(forward, vec![]);
            assert_eq!(reverse, vec![]);
            Ok(())
        })
    }

    /// TODO break up into smaller tests
    #[tokio::test(threaded_scheduler)]
    async fn kv_store_sanity_check() -> DatabaseResult<()> {
        let arc = test_cell_env().await;
        let env = arc.guard().await;
        let db1 = env.inner().open_single("kv1", StoreOptions::create())?;
        let db2 = env.inner().open_single("kv1", StoreOptions::create())?;

        let testval = TestVal {
            name: "Joe".to_owned(),
        };

        env.with_reader::<DatabaseError, _, _>(|reader| {
            let mut kv1: KvBuf<String, TestVal> = KvBuf::new(&reader, db1)?;
            let mut kv2: KvBuf<String, String> = KvBuf::new(&reader, db2)?;

            env.with_commit(|writer| {
                kv1.put("hi".to_owned(), testval.clone());
                kv2.put("salutations".to_owned(), "folks".to_owned());

                // Check that the underlying store contains no changes yet
                assert_eq!(kv1.get_persisted(&"hi".to_owned())?, None);
                assert_eq!(kv2.get_persisted(&"salutations".to_owned())?, None);
                kv1.flush_to_txn(writer)
            })?;

            // Ensure that mid-transaction, there has still been no persistence,
            // just for kicks

            env.with_commit(|writer| {
                let kv1a: KvBuf<String, TestVal> = KvBuf::new(&reader, db1)?;
                assert_eq!(kv1a.get_persisted(&"hi".to_owned())?, None);
                kv2.flush_to_txn(writer)
            })
        })?;

        env.with_reader(|reader| {
            // Now open some fresh Readers to see that our data was persisted
            let kv1b: KvBuf<String, TestVal> = KvBuf::new(&reader, db1)?;
            let kv2b: KvBuf<String, String> = KvBuf::new(&reader, db2)?;
            // Check that the underlying store contains no changes yet
            assert_eq!(kv1b.get_persisted(&"hi".to_owned())?, Some(testval));
            assert_eq!(
                kv2b.get_persisted(&"salutations".to_owned())?,
                Some("folks".to_owned())
            );
            Ok(())
        })
    }

    #[tokio::test(threaded_scheduler)]
    async fn kv_indicate_value_overwritten() -> DatabaseResult<()> {
        holochain_types::observability::test_run().ok();
        let arc = test_cell_env().await;
        let env = arc.guard().await;
        let db = env.inner().open_single("kv", StoreOptions::create())?;
        env.with_reader(|reader| {
            let mut buf = KvBuf::new(&reader, db)?;

            buf.put("a", V(1));
            assert_eq!(Some(V(1)), buf.get(&"a")?);
            buf.put("a", V(2));
            assert_eq!(Some(V(2)), buf.get(&"a")?);
            Ok(())
        })
    }

    #[tokio::test(threaded_scheduler)]
    async fn kv_deleted_persisted() -> DatabaseResult<()> {
        use tracing::*;
        holochain_types::observability::test_run().ok();
        let arc = test_cell_env().await;
        let env = arc.guard().await;
        let db = env.inner().open_single("kv", StoreOptions::create())?;

        env.with_reader(|reader| {
            let mut buf = KvBuf::new(&reader, db).unwrap();

            buf.put("a", V(1));
            buf.put("b", V(2));
            buf.put("c", V(3));

            env.with_commit(|mut writer| buf.flush_to_txn(&mut writer))
        })?;
        env.with_reader(|reader| {
            let mut buf: KvBuf<_, V> = KvBuf::new(&reader, db).unwrap();

            buf.delete("b");

            env.with_commit(|mut writer| buf.flush_to_txn(&mut writer))
        })?;
        env.with_reader(|reader| {
            let buf: KvBuf<&str, _> = KvBuf::new(&reader, db).unwrap();

            let forward: Vec<_> = buf.iter_raw().unwrap().collect();
            debug!(?forward);
            assert_eq!(forward, vec![(&b"a"[..], V(1)), (&b"c"[..], V(3))],);
            Ok(())
        })
    }

    #[tokio::test(threaded_scheduler)]
    async fn kv_deleted_buffer() -> DatabaseResult<()> {
        holochain_types::observability::test_run().ok();
        let arc = test_cell_env().await;
        let env = arc.guard().await;
        let db = env.inner().open_single("kv", StoreOptions::create())?;

        env.with_reader(|reader| {
            let mut buf = KvBuf::new(&reader, db).unwrap();

            buf.put("a", V(5));
            buf.put("b", V(4));
            buf.put("c", V(9));
            test_buf(
                &buf.scratch,
                [res!("a", Put, 5), res!("b", Put, 4), res!("c", Put, 9)]
                    .iter()
                    .cloned(),
            );
            buf.delete("b");
            test_buf(
                &buf.scratch,
                [res!("a", Put, 5), res!("c", Put, 9), res!("b", Delete)]
                    .iter()
                    .cloned(),
            );

            env.with_commit(|mut writer| buf.flush_to_txn(&mut writer))
        })?;
        env.with_reader(|reader| {
            let buf: KvBuf<&str, _> = KvBuf::new(&reader, db).unwrap();

            let forward: Vec<_> = buf.iter_raw().unwrap().collect();
            assert_eq!(forward, vec![(&b"a"[..], V(5)), (&b"c"[..], V(9))]);
            Ok(())
        })
    }

    #[tokio::test(threaded_scheduler)]
    async fn kv_get_buffer() -> DatabaseResult<()> {
        holochain_types::observability::test_run().ok();
        let arc = test_cell_env().await;
        let env = arc.guard().await;
        let db = env.inner().open_single("kv", StoreOptions::create())?;

        env.with_reader(|reader| {
            let mut buf = KvBuf::new(&reader, db).unwrap();

            buf.put("a", V(5));
            buf.put("b", V(4));
            buf.put("c", V(9));
            let n = buf.get(&"b")?;
            assert_eq!(n, Some(V(4)));

            Ok(())
        })
    }

    #[tokio::test(threaded_scheduler)]
    async fn kv_get_persisted() -> DatabaseResult<()> {
        holochain_types::observability::test_run().ok();
        let arc = test_cell_env().await;
        let env = arc.guard().await;
        let db = env.inner().open_single("kv", StoreOptions::create())?;

        env.with_reader(|reader| {
            let mut buf = KvBuf::new(&reader, db).unwrap();

            buf.put("a", V(1));
            buf.put("b", V(2));
            buf.put("c", V(3));

            env.with_commit(|mut writer| buf.flush_to_txn(&mut writer))
        })?;

        env.with_reader(|reader| {
            let buf = KvBuf::new(&reader, db).unwrap();

            let n = buf.get(&"b")?;
            assert_eq!(n, Some(V(2)));
            Ok(())
        })
    }

    #[tokio::test(threaded_scheduler)]
    async fn kv_get_del_buffer() -> DatabaseResult<()> {
        holochain_types::observability::test_run().ok();
        let arc = test_cell_env().await;
        let env = arc.guard().await;
        let db = env.inner().open_single("kv", StoreOptions::create())?;

        env.with_reader(|reader| {
            let mut buf = KvBuf::new(&reader, db).unwrap();

            buf.put("a", V(5));
            buf.put("b", V(4));
            buf.put("c", V(9));
            buf.delete("b");
            let n = buf.get(&"b")?;
            assert_eq!(n, None);
            Ok(())
        })
    }

    #[tokio::test(threaded_scheduler)]
    async fn kv_get_del_persisted() -> DatabaseResult<()> {
        holochain_types::observability::test_run().ok();
        let arc = test_cell_env().await;
        let env = arc.guard().await;
        let db = env.inner().open_single("kv", StoreOptions::create())?;

        env.with_reader(|reader| {
            let mut buf = KvBuf::new(&reader, db).unwrap();

            buf.put("a", V(1));
            buf.put("b", V(2));
            buf.put("c", V(3));

            env.with_commit(|mut writer| buf.flush_to_txn(&mut writer))
        })?;

        env.with_reader(|reader| {
            let mut buf: KvBuf<_, V> = KvBuf::new(&reader, db).unwrap();

            buf.delete("b");
            let n = buf.get(&"b")?;
            assert_eq!(n, None);

            env.with_commit(|mut writer| buf.flush_to_txn(&mut writer))
        })?;

        env.with_reader(|reader| {
            let buf: KvBuf<_, V> = KvBuf::new(&reader, db).unwrap();

            let n = buf.get(&"b")?;
            assert_eq!(n, None);
            Ok(())
        })
    }
}
