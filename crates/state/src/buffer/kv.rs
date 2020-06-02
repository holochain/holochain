use super::{BufKey, BufVal, BufferedStore};
use crate::{
    error::{DatabaseError, DatabaseResult},
    prelude::{Readable, Reader, Writer},
};
use rkv::SingleStore;

use std::{collections::BTreeMap, marker::PhantomData};

use fallible_iterator::FallibleIterator;
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
    scratch: BTreeMap<Vec<u8>, Op<V>>,
    __key: PhantomData<K>,
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
            scratch: BTreeMap::new(),
            __key: PhantomData,
        })
    }

    /// Get a value, taking the scratch space into account,
    /// or from persistence if needed
    pub fn get(&self, k: &K) -> DatabaseResult<Option<V>> {
        // Empty keys break lmdb
        if k.as_ref().len() < 1 {
            return Ok(None);
        }
        use Op::*;
        let val = match self.scratch.get(k.as_ref()) {
            Some(Put(scratch_val)) => Some(*scratch_val.clone()),
            Some(Delete) => None,
            None => self.get_persisted(k)?,
        };
        Ok(val)
    }

    /// Update the scratch space to record a Put operation for the KV
    pub fn put(&mut self, k: K, v: V) {
        let k = k.as_ref().to_vec();
        // Empty keys break lmdb
        if k.len() < 1 {
            return;
        }
        self.scratch.insert(k, Op::Put(Box::new(v)));
    }

    /// Update the scratch space to record a Delete operation for the KV
    pub fn delete(&mut self, k: K) {
        let k = k.as_ref().to_vec();
        // Empty keys break lmdb
        if k.len() < 1 {
            return;
        }
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

    /// Iterator that checks the scratch space
    pub fn iter(&self) -> DatabaseResult<SingleIter<V>> {
        Ok(SingleIter::new(
            &self.scratch,
            self.scratch.iter(),
            self.iter_raw()?,
        ))
    }

    /// Iterate from a key onwards
    pub fn iter_from(&self, k: K) -> DatabaseResult<SingleFromIter<V>> {
        let key = k.as_ref().to_vec();
        Ok(SingleFromIter::new(
            &self.scratch,
            self.iter_raw_from(k)?,
            key,
        ))
    }

    /// Iterate over the data in reverse
    pub fn iter_reverse(&self) -> DatabaseResult<SingleIter<V>> {
        Ok(SingleIter::new(
            &self.scratch,
            self.scratch.iter().rev(),
            self.iter_raw_reverse()?,
        ))
    }

    /// Iterate over the underlying persisted data, NOT taking the scratch space into consideration
    pub fn iter_raw(&self) -> DatabaseResult<SingleIterRaw<V>> {
        Ok(SingleIterRaw::new(self.db.iter_start(self.reader)?))
    }

    /// Iterate from a key onwards without scratch space
    pub fn iter_raw_from(&self, k: K) -> DatabaseResult<SingleIterRaw<V>> {
        Ok(SingleIterRaw::new(self.db.iter_from(self.reader, k)?))
    }

    /// Iterate over the underlying persisted data in reverse, NOT taking the scratch space into consideration
    pub fn iter_raw_reverse(&self) -> DatabaseResult<SingleIterRaw<V>> {
        Ok(SingleIterRaw::new(self.db.iter_end(self.reader)?))
    }

    /// Iterate over items which are staged for PUTs in the scratch space
    // HACK: unfortunate leaky abstraction here, but needed to allow comprehensive
    // iteration, by chaining this with an iter_raw
    pub fn iter_scratch_puts(&self) -> impl Iterator<Item = (&Vec<u8>, &Box<V>)> {
        self.scratch.iter().filter_map(|(k, op)| {
            if let Op::Put(v) = op {
                Some((k, v))
            } else {
                None
            }
        })
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

/// Match a key on another partial key
pub fn partial_key_match(partial_key: &[u8], key: &[u8]) -> bool {
    let len = partial_key.len();
    // Avoid slice panic
    key.get(0..len)
        .map(|a| a == &partial_key[..])
        .unwrap_or(false)
}
pub struct SingleFromIter<'env, 'a, V>
where
    V: BufVal,
{
    iter: SingleIter<'env, 'a, V>,
}

impl<'env, 'a: 'env, V> SingleFromIter<'env, 'a, V>
where
    V: BufVal,
{
    fn new(
        scratch: &'a BTreeMap<Vec<u8>, Op<V>>,
        iter: SingleIterRaw<'env, V>,
        key: Vec<u8>,
    ) -> Self {
        let iter = SingleIter::new(&scratch, scratch.range(key..), iter);
        Self { iter }
    }
}

impl<'env, 'a: 'env, V> FallibleIterator for SingleFromIter<'env, 'a, V>
where
    V: BufVal,
{
    type Error = DatabaseError;
    type Item = (&'env [u8], V);
    #[instrument(skip(self))]
    fn next(&mut self) -> Result<Option<Self::Item>, Self::Error> {
        self.iter.next()
    }
}

pub struct SingleIter<'env, 'a, V>
where
    V: BufVal,
{
    scratch_iter: Box<dyn Iterator<Item = (&'a [u8], V)> + 'a>,
    iter: Box<dyn FallibleIterator<Item = (&'env [u8], V), Error = DatabaseError> + 'env>,
    scratch_current: Option<(&'a [u8], V)>,
    current: Option<(&'env [u8], V)>,
}

impl<'env, 'a: 'env, V> SingleIter<'env, 'a, V>
where
    V: BufVal,
{
    fn new(
        scratch: &'a BTreeMap<Vec<u8>, Op<V>>,
        scratch_iter: impl Iterator<Item = (&'a Vec<u8>, &'a Op<V>)> + 'a,
        iter: SingleIterRaw<'env, V>,
    ) -> Self {
        let scratch_iter = scratch_iter
            .inspect(|(k, v)| {
                let span = trace_span!("scratch < filter", key = %String::from_utf8_lossy(k));
                let _g = span.enter();
                trace!(k = %String::from_utf8_lossy(k), ?v)
            })
            .filter_map(|(k, v)| match v {
                Op::Put(v) => Some((&k[..], *v.clone())),
                Op::Delete => None,
            })
            .inspect(|(k, v)| {
                let span = trace_span!("scratch > filter", key = %String::from_utf8_lossy(k));
                let _g = span.enter();
                trace!(k = %String::from_utf8_lossy(k), ?v)
            });
        let iter = iter
            .inspect(|(k, v)| {
                let span = trace_span!("db < filter", key = %String::from_utf8_lossy(k));
                let _g = span.enter();
                trace!(k = %String::from_utf8_lossy(k), ?v);
                Ok(())
            })
            .filter_map(move |(k, v)| match scratch.get(k) {
                Some(Op::Put(sv)) => Ok(Some((k, *sv.clone()))),
                Some(Op::Delete) => Ok(None),
                None => Ok(Some((k, v))),
            })
            .inspect(|(k, v)| {
                let span = trace_span!("db > filter", key = %String::from_utf8_lossy(k));
                let _g = span.enter();
                trace!(k = %String::from_utf8_lossy(k), ?v);
                Ok(())
            });
        Self {
            scratch_iter: Box::new(scratch_iter),
            iter: Box::new(iter),
            current: None,
            scratch_current: None,
        }
    }
}

impl<'env, 'a: 'env, V> FallibleIterator for SingleIter<'env, 'a, V>
where
    V: BufVal,
{
    type Error = DatabaseError;
    type Item = (&'env [u8], V);
    #[instrument(skip(self))]
    fn next(&mut self) -> Result<Option<Self::Item>, Self::Error> {
        let current = match self.current.take() {
            Some(c) => Some(c),
            None => self.iter.next()?,
        };
        let scratch_current = match self.scratch_current.take() {
            Some(c) => Some(c),
            None => self.scratch_iter.next(),
        };
        let r = match current {
            Some(db) => match scratch_current {
                Some(scratch) if scratch.0 < db.0 => {
                    trace!(msg = "r scratch key <", k = %String::from_utf8_lossy(&scratch.0[..]), v = ?scratch.1);
                    self.current = Some(db);
                    Some(scratch)
                }
                Some(scratch) if scratch.0 == db.0 => {
                    trace!(msg = "r scratch key ==", k = %String::from_utf8_lossy(&scratch.0[..]), v = ?scratch.1);
                    Some(scratch)
                }
                _ => {
                    trace!(msg = "r db _", k = %String::from_utf8_lossy(&db.0[..]), v = ?db.1);
                    self.scratch_current = scratch_current;
                    Some(db)
                }
            },
            None => {
                if let Some((k, v)) = &scratch_current {
                    trace!(msg = "r scratch no db", k = %String::from_utf8_lossy(k), ?v);
                } else {
                    trace!("r None")
                }
                scratch_current
            }
        };
        Ok(r)
    }
}

pub struct SingleIterRaw<'env, V>(rkv::store::single::Iter<'env>, std::marker::PhantomData<V>);

impl<'env, V> SingleIterRaw<'env, V> {
    pub fn new(iter: rkv::store::single::Iter<'env>) -> Self {
        Self(iter, std::marker::PhantomData)
    }
}

/// Iterate over key, value pairs in this store using low-level LMDB iterators
/// NOTE: While the value is deserialized to the proper type, the key is returned as raw bytes.
/// This is to enable a wider range of keys, such as String, because there is no uniform trait which
/// enables conversion from a byte slice to a given type.
impl<'env, V> FallibleIterator for SingleIterRaw<'env, V>
where
    V: BufVal,
{
    type Error = DatabaseError;
    type Item = (&'env [u8], V);

    fn next(&mut self) -> Result<Option<Self::Item>, Self::Error> {
        match self.0.next() {
            Some(Ok((k, Some(rkv::Value::Blob(buf))))) => Ok(Some((
                k,
                rmp_serde::from_read_ref(buf).expect(
                    "Failed to deserialize data from database. Database might be corrupted",
                ),
            ))),
            None => Ok(None),
            // TODO: Should this panic aswell?
            Some(Ok(_)) => Err(DatabaseError::InvalidValue),
            // This could be a IO error so returning it makes sense
            Some(Err(e)) => Err(DatabaseError::from(e)),
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
    use fallible_iterator::FallibleIterator;
    use fixt::prelude::*;
    use rkv::StoreOptions;
    use serde_derive::{Deserialize, Serialize};
    use std::collections::BTreeMap;
    use tracing::*;

    #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
    struct TestVal {
        name: String,
    }

    #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
    pub struct V(u32);

    impl From<u32> for V {
        fn from(s: u32) -> Self {
            Self(s)
        }
    }

    fixturator!(V; from u32;);

    type TestBuf<'a> = KvBuf<'a, &'a str, V>;

    macro_rules! res {
        ($key:expr, $op:ident, $val:expr) => {
            ($key, Op::$op(Box::new(V($val))))
        };
        ($key:expr, $op:ident) => {
            ($key, Op::$op)
        };
    }

    fn test_buf(a: &BTreeMap<Vec<u8>, Op<V>>, b: impl Iterator<Item = (&'static str, Op<V>)>) {
        for (k, v) in b {
            let val = a.get(k.as_bytes()).expect("Missing key");
            assert_eq!(*val, v);
        }
    }

    #[tokio::test(threaded_scheduler)]
    async fn kv_iterators() -> DatabaseResult<()> {
        let arc = test_cell_env();
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

            let forward: Vec<_> = buf.iter_raw()?.map(|(_, v)| Ok(v)).collect().unwrap();
            let reverse: Vec<_> = buf
                .iter_raw_reverse()?
                .map(|(_, v)| Ok(v))
                .collect()
                .unwrap();

            assert_eq!(forward, vec![V(1), V(2), V(3), V(4), V(5)]);
            assert_eq!(reverse, vec![V(5), V(4), V(3), V(2), V(1)]);
            Ok(())
        })
    }

    #[tokio::test(threaded_scheduler)]
    async fn kv_empty_iterators() -> DatabaseResult<()> {
        let arc = test_cell_env();
        let env = arc.guard().await;
        let db = env
            .inner()
            .open_single("kv", StoreOptions::create())
            .unwrap();

        env.with_reader(|reader| {
            let buf: TestBuf = KvBuf::new(&reader, db).unwrap();

            let forward: Vec<_> = buf.iter_raw().unwrap().collect().unwrap();
            let reverse: Vec<_> = buf.iter_raw_reverse().unwrap().collect().unwrap();

            assert_eq!(forward, vec![]);
            assert_eq!(reverse, vec![]);
            Ok(())
        })
    }

    /// TODO break up into smaller tests
    #[tokio::test(threaded_scheduler)]
    async fn kv_store_sanity_check() -> DatabaseResult<()> {
        let arc = test_cell_env();
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
        let arc = test_cell_env();
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
        let arc = test_cell_env();
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

            let forward = buf.iter_raw().unwrap().collect::<Vec<_>>().unwrap();
            debug!(?forward);
            assert_eq!(forward, vec![(&b"a"[..], V(1)), (&b"c"[..], V(3))],);
            Ok(())
        })
    }

    #[tokio::test(threaded_scheduler)]
    async fn kv_deleted_buffer() -> DatabaseResult<()> {
        holochain_types::observability::test_run().ok();
        let arc = test_cell_env();
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

            let forward: Vec<_> = buf.iter_raw().unwrap().collect().unwrap();
            assert_eq!(forward, vec![(&b"a"[..], V(5)), (&b"c"[..], V(9))]);
            Ok(())
        })
    }

    #[tokio::test(threaded_scheduler)]
    async fn kv_get_buffer() -> DatabaseResult<()> {
        holochain_types::observability::test_run().ok();
        let arc = test_cell_env();
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
        let arc = test_cell_env();
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
        let arc = test_cell_env();
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
        let arc = test_cell_env();
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

    #[tokio::test(threaded_scheduler)]
    async fn kv_iter_from_partial() {
        let arc = test_cell_env();
        let env = arc.guard().await;
        let db = env
            .inner()
            .open_single("kv", StoreOptions::create())
            .unwrap();

        env.with_reader::<DatabaseError, _, _>(|reader| {
            let mut buf: TestBuf = KvBuf::new(&reader, db).unwrap();

            buf.put("a", V(101));
            buf.put("b", V(102));
            buf.put("dogs_likes_7", V(1));
            buf.put("dogs_likes_79", V(2));
            buf.put("dogs_likes_3", V(3));
            buf.put("dogs_likes_88", V(4));
            buf.put("dogs_likes_f", V(5));
            buf.put("d", V(103));
            buf.put("e", V(104));
            buf.put("aaaaaaaaaaaaaaaaaaaa", V(105));
            buf.put("eeeeeeeeeeeeeeeeeeee", V(106));

            env.with_commit(|mut writer| buf.flush_to_txn(&mut writer))
                .unwrap();
            Ok(())
        })
        .unwrap();

        env.with_reader::<DatabaseError, _, _>(|reader| {
            let buf: TestBuf = KvBuf::new(&reader, db).unwrap();

            let iter = buf.iter_raw_from("dogs_likes").unwrap();
            let results = iter.collect::<Vec<_>>().unwrap();
            assert_eq!(
                results,
                vec![
                    (&b"dogs_likes_3"[..], V(3)),
                    (&b"dogs_likes_7"[..], V(1)),
                    (&b"dogs_likes_79"[..], V(2)),
                    (&b"dogs_likes_88"[..], V(4)),
                    (&b"dogs_likes_f"[..], V(5)),
                    (&b"e"[..], V(104)),
                    (&b"eeeeeeeeeeeeeeeeeeee"[..], V(106)),
                ]
            );

            Ok(())
        })
        .unwrap();
    }

    enum TestData {
        Put((String, V)),
        Del(String),
    }

    fn do_test(
        buf: &mut KvBuf<String, V>,
        puts_dels_iter: &mut impl Iterator<Item = TestData>,
        expected_state: &mut BTreeMap<String, V>,
        runs: &mut Vec<String>,
        reproduce: &mut Vec<String>,
        from_key: &String,
    ) {
        let mut rng = rand::thread_rng();
        for _ in 0..rng.gen_range(1, 300) {
            match puts_dels_iter.next() {
                Some(TestData::Put((key, value))) => {
                    runs.push(format!("Put: key: {}, val: {:?} -> ", key, value));
                    reproduce.push(format!(
                        "TestData::Put(({:?}.to_string(), {:?})), ",
                        key, value
                    ));
                    buf.put(key.clone(), value.clone());
                    expected_state.insert(key, value);
                }
                Some(TestData::Del(key)) => {
                    runs.push(format!("Del: key: {} -> ", key));
                    reproduce.push(format!("TestData::Del({:?}.to_string()), ", key));
                    buf.delete(key.clone());
                    expected_state.remove(&key);
                }
                None => break,
            }
        }
        expected_state.remove("");
        assert_eq!(
            buf.iter()
                .unwrap()
                .map(|(k, v)| Ok((String::from_utf8(k.to_vec()).unwrap(), v)))
                .inspect(|(k, v)| Ok(trace!(?k, ?v)))
                .collect::<Vec<_>>()
                .unwrap(),
            expected_state
                .iter()
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect::<Vec<_>>(),
            "{}\n{}];",
            runs.concat(),
            reproduce.concat()
        );
        assert_eq!(
            buf.iter_from(from_key.clone())
                .unwrap()
                .map(|(k, v)| Ok((String::from_utf8(k.to_vec()).unwrap(), v)))
                .inspect(|(k, v)| Ok(trace!(?k, ?v)))
                .collect::<Vec<_>>()
                .unwrap(),
            expected_state
                .range::<String, _>(from_key..)
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect::<Vec<_>>(),
            "{}\n{}];",
            runs.concat(),
            reproduce.concat()
        );
    }

    fn re_do_test(
        buf: &mut KvBuf<String, V>,
        puts_dels_iter: &mut impl Iterator<Item = TestData>,
        expected_state: &mut BTreeMap<String, V>,
        runs: &mut Vec<String>,
    ) {
        while let Some(td) = puts_dels_iter.next() {
            match td {
                TestData::Put((key, value)) => {
                    runs.push(format!("Put: key: {}, val: {:?} -> ", key, value));
                    buf.put(key.clone(), value.clone());
                    expected_state.insert(key, value);
                }
                TestData::Del(key) => {
                    runs.push(format!("Del: key: {} -> ", key));
                    buf.delete(key.clone());
                    expected_state.remove(&key);
                }
            }
        }
        expected_state.remove("");
        assert_eq!(
            buf.iter()
                .unwrap()
                .map(|(k, v)| Ok((String::from_utf8(k.to_vec()).unwrap(), v)))
                .inspect(|(k, v)| Ok(trace!(?k, ?v)))
                .collect::<Vec<_>>()
                .unwrap(),
            expected_state
                .iter()
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect::<Vec<_>>(),
            "{}",
            runs.concat(),
        );
    }

    #[tokio::test(threaded_scheduler)]
    async fn kv_single_iter() {
        holochain_types::observability::test_run().ok();
        let mut rng = rand::thread_rng();
        let arc = test_cell_env();
        let env = arc.guard().await;
        let db = env
            .inner()
            .open_single("kv", StoreOptions::create())
            .unwrap();
        let td = StringFixturator::new(Unpredictable)
            .zip(VFixturator::new(Unpredictable))
            .take(300)
            .collect::<BTreeMap<_, _>>();
        let td_vec = td.into_iter().collect::<Vec<_>>();
        let mut puts = rng
            .sample_iter(rand::distributions::Uniform::new(0, td_vec.len()))
            .map(|i| td_vec[i].clone());
        let from_key = puts.next().unwrap().0;
        let mut dels = rng
            .sample_iter(rand::distributions::Uniform::new(0, td_vec.len()))
            .map(|i| td_vec[i].0.clone());
        let puts_dels = puts
            .map(|p| {
                if rng.gen() {
                    TestData::Put(p)
                } else {
                    TestData::Del(dels.next().unwrap())
                }
            })
            .take(1000)
            .collect::<Vec<_>>();
        let mut puts_dels = puts_dels.into_iter();
        let mut expected_state: BTreeMap<String, V> = BTreeMap::new();

        let span = trace_span!("kv_single_iter");
        let _g = span.enter();

        let mut runs = vec!["Start | ".to_string()];
        let mut reproduce = vec!["\nReproduce:\n".to_string()];

        env.with_reader::<DatabaseError, _, _>(|reader| {
            let mut buf: KvBuf<String, V> = KvBuf::new(&reader, db).unwrap();
            let span = trace_span!("in_scratch");
            let _g = span.enter();
            runs.push(format!(
                "{} | ",
                span.metadata().map(|m| m.name()).unwrap_or("")
            ));
            reproduce.push(format!(
                "let {} = [",
                span.metadata().map(|f| f.name()).unwrap_or("")
            ));
            do_test(
                &mut buf,
                &mut puts_dels,
                &mut expected_state,
                &mut runs,
                &mut reproduce,
                &from_key,
            );
            env.with_commit(|mut writer| buf.flush_to_txn(&mut writer))
                .unwrap();
            Ok(())
        })
        .unwrap();

        env.with_reader::<DatabaseError, _, _>(|reader| {
            let mut buf: KvBuf<String, V> = KvBuf::new(&reader, db).unwrap();
            let span = trace_span!("in_db_first");
            let _g = span.enter();
            runs.push(format!(
                "{} | ",
                span.metadata().map(|m| m.name()).unwrap_or("")
            ));
            reproduce.push(format!(
                "]; \n\nlet {} = [",
                span.metadata().map(|f| f.name()).unwrap_or("")
            ));
            do_test(
                &mut buf,
                &mut puts_dels,
                &mut expected_state,
                &mut runs,
                &mut reproduce,
                &from_key,
            );
            env.with_commit(|mut writer| buf.flush_to_txn(&mut writer))
                .unwrap();
            Ok(())
        })
        .unwrap();
        env.with_reader::<DatabaseError, _, _>(|reader| {
            let mut buf: KvBuf<String, V> = KvBuf::new(&reader, db).unwrap();
            let span = trace_span!("in_db_second");
            let _g = span.enter();
            runs.push(format!(
                "{} | ",
                span.metadata().map(|m| m.name()).unwrap_or("")
            ));
            reproduce.push(format!(
                "]; \n\nlet {} = [",
                span.metadata().map(|f| f.name()).unwrap_or("")
            ));
            do_test(
                &mut buf,
                &mut puts_dels,
                &mut expected_state,
                &mut runs,
                &mut reproduce,
                &from_key,
            );
            env.with_commit(|mut writer| buf.flush_to_txn(&mut writer))
                .unwrap();
            Ok(())
        })
        .unwrap();
    }

    #[tokio::test(threaded_scheduler)]
    async fn kv_single_iter_found_1() {
        holochain_types::observability::test_run().ok();
        let in_scratch = vec![
            TestData::Del(".".to_string()),
            TestData::Put(("bar".to_string(), V(0))),
            TestData::Put(("!".to_string(), V(0))),
            TestData::Put(("💯".to_string(), V(889149878))),
            TestData::Put(("bing".to_string(), V(823748021))),
            TestData::Put(("foo".to_string(), V(3698192405))),
            TestData::Del("💯".to_string()),
            TestData::Del("bing".to_string()),
            TestData::Put(("baz".to_string(), V(3224166057))),
            TestData::Del("💩".to_string()),
            TestData::Put(("baz".to_string(), V(3224166057))),
            TestData::Put((".".to_string(), V(0))),
            TestData::Del("❤".to_string()),
            TestData::Put(("💯".to_string(), V(889149878))),
            TestData::Put((".".to_string(), V(0))),
            TestData::Del("bing".to_string()),
            TestData::Put(("!".to_string(), V(0))),
            TestData::Del("!".to_string()),
            TestData::Del(".".to_string()),
        ];
        let in_db_first = vec![
            TestData::Del("!".to_string()),
            TestData::Del("foo".to_string()),
            TestData::Del("foo".to_string()),
            TestData::Put(("foo".to_string(), V(3698192405))),
            TestData::Put(("bar".to_string(), V(0))),
            TestData::Del("!".to_string()),
            TestData::Del("💩".to_string()),
            TestData::Put(("bar".to_string(), V(0))),
        ];
        let in_db_second = vec![];
        let span = trace_span!("kv_single_iter_found_1");
        let _g = span.enter();
        kv_single_iter_runner(
            in_scratch.into_iter(),
            in_db_first.into_iter(),
            in_db_second.into_iter(),
        )
        .await;
    }

    #[tokio::test(threaded_scheduler)]
    async fn kv_single_iter_found_2() {
        holochain_types::observability::test_run().ok();
        let in_scratch = vec![
            TestData::Del("".to_string()),
            TestData::Put(("".to_string(), V(0))),
        ];
        let in_db_first = vec![
            TestData::Del("".to_string()),
            TestData::Put(("".to_string(), V(2))),
        ];
        let in_db_second = vec![
            TestData::Del("".to_string()),
            TestData::Put(("".to_string(), V(2))),
        ];
        let span = trace_span!("kv_single_iter_found_2");
        let _g = span.enter();
        kv_single_iter_runner(
            in_scratch.into_iter(),
            in_db_first.into_iter(),
            in_db_second.into_iter(),
        )
        .await;
    }

    #[tokio::test(threaded_scheduler)]
    async fn kv_single_iter_found_3() {
        holochain_types::observability::test_run().ok();
        let in_scratch = vec![
            TestData::Put(("m".to_string(), V(0))),
            TestData::Put(("n".to_string(), V(0))),
            TestData::Put(("o".to_string(), V(0))),
            TestData::Put(("o".to_string(), V(0))),
            TestData::Put(("p".to_string(), V(0))),
        ];
        let in_db_first = vec![
            TestData::Put(("o".to_string(), V(2))),
            TestData::Put(("o".to_string(), V(2))),
            TestData::Put(("o".to_string(), V(2))),
        ];
        let in_db_second = vec![
            TestData::Put(("o".to_string(), V(2))),
            TestData::Del("o".to_string()),
            TestData::Put(("o".to_string(), V(2))),
        ];
        let span = trace_span!("kv_single_iter_found_3");
        let _g = span.enter();
        kv_single_iter_runner(
            in_scratch.into_iter(),
            in_db_first.into_iter(),
            in_db_second.into_iter(),
        )
        .await;
    }

    async fn kv_single_iter_runner(
        in_scratch: impl Iterator<Item = TestData> + Send,
        in_db_first: impl Iterator<Item = TestData> + Send,
        in_db_second: impl Iterator<Item = TestData> + Send,
    ) {
        let arc = test_cell_env();
        let env = arc.guard().await;
        let db = env
            .inner()
            .open_single("kv", StoreOptions::create())
            .unwrap();

        let mut runs = vec!["Start | ".to_string()];
        let mut expected_state: BTreeMap<String, V> = BTreeMap::new();

        env.with_reader::<DatabaseError, _, _>(|reader| {
            let mut buf: KvBuf<String, V> = KvBuf::new(&reader, db).unwrap();
            let span = trace_span!("in_scratch");
            let _g = span.enter();
            runs.push(format!(
                "{} | ",
                span.metadata().map(|m| m.name()).unwrap_or("in_scratch")
            ));
            re_do_test(
                &mut buf,
                &mut in_scratch.into_iter(),
                &mut expected_state,
                &mut runs,
            );
            env.with_commit(|mut writer| buf.flush_to_txn(&mut writer))
                .unwrap();
            Ok(())
        })
        .unwrap();

        env.with_reader::<DatabaseError, _, _>(|reader| {
            let mut buf: KvBuf<String, V> = KvBuf::new(&reader, db).unwrap();
            let span = trace_span!("in_db_first");
            let _g = span.enter();
            runs.push(format!(
                "{} | ",
                span.metadata().map(|m| m.name()).unwrap_or("in_db_first")
            ));
            re_do_test(
                &mut buf,
                &mut in_db_first.into_iter(),
                &mut expected_state,
                &mut runs,
            );
            env.with_commit(|mut writer| buf.flush_to_txn(&mut writer))
                .unwrap();
            Ok(())
        })
        .unwrap();

        env.with_reader::<DatabaseError, _, _>(|reader| {
            let mut buf: KvBuf<String, V> = KvBuf::new(&reader, db).unwrap();
            let span = trace_span!("in_db_second");
            let _g = span.enter();
            runs.push(format!(
                "{} | ",
                span.metadata().map(|m| m.name()).unwrap_or("in_db_second")
            ));
            re_do_test(
                &mut buf,
                &mut in_db_second.into_iter(),
                &mut expected_state,
                &mut runs,
            );
            env.with_commit(|mut writer| buf.flush_to_txn(&mut writer))
                .unwrap();
            Ok(())
        })
        .unwrap();
    }
}
