use super::{BufKey, BufVal, BufferedStore};
use crate::{
    error::{DatabaseError, DatabaseResult},
    prelude::{Readable, Reader, Writer},
};
use rkv::SingleStore;

use std::{collections::BTreeMap, marker::PhantomData};

use fallible_iterator::FallibleIterator;

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
        self.scratch
            .insert(k.as_ref().to_vec(), Op::Put(Box::new(v)));
    }

    /// Update the scratch space to record a Delete operation for the KV
    pub fn delete(&mut self, k: K) {
        self.scratch.insert(k.as_ref().to_vec(), Op::Delete);
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
        Ok(SingleIter::new(&self.scratch, self.iter_raw()?))
    }

    /// Iterate from a key onwards
    pub fn iter_from(&self, k: K) -> DatabaseResult<SingleFromIter<V>> {
        let key = k.as_ref().to_vec();
        Ok(SingleFromIter::new(
            SingleIter::new(&self.scratch, self.iter_raw_from(k)?),
            key,
        ))
    }

    /// Iterate over the data in reverse
    pub fn iter_reverse(&self) -> DatabaseResult<SingleIter<V>> {
        Ok(SingleIter::new(&self.scratch, self.iter_raw_reverse()?))
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
type KeyVal<'a, V> = (&'a Vec<u8>, V);
type KeyValSlice<'env, V> = (&'env [u8], V);
pub struct SingleFromIter<'env, 'a, V> {
    partial_matches: std::collections::btree_map::IntoIter<&'a Vec<u8>, V>,
    iter: SingleIter<'env, 'a, V>,
    current: Option<DatabaseResult<KeyValSlice<'env, V>>>,
    current_scratch: Option<KeyVal<'a, V>>,
}

impl<'env, 'a, V> SingleFromIter<'env, 'a, V>
where
    V: BufVal,
{
    fn new(iter: SingleIter<'env, 'a, V>, key: Vec<u8>) -> Self {
        let mut deletes = Vec::new();
        let mut partial_matches: BTreeMap<_, V> = iter
            .scratch
            .iter()
            .skip_while(|(k, _)| !partial_key_match(&key[..], k))
            .take_while(|(k, _)| partial_key_match(&key[..], k))
            .filter_map(|(k, v)| match v {
                Op::Put(v) => Some((k, (**v).clone())),
                Op::Delete => {
                    deletes.push(k);
                    None
                }
            })
            .collect();
        for delete in deletes {
            partial_matches.remove(delete);
        }
        Self {
            iter,
            partial_matches: partial_matches.into_iter(),
            current: None,
            current_scratch: None,
        }
    }
}

impl<'env, 'a, V> Iterator for SingleFromIter<'env, 'a, V>
where
    V: BufVal,
{
    type Item = Result<(Vec<u8>, V), DatabaseError>;
    fn next(&mut self) -> Option<Self::Item> {
        let current = match self.current.take() {
            Some(s) => Some(s),
            None => Iterator::next(&mut self.iter),
        };
        let current_scratch = match self.current_scratch.take() {
            Some(s) => Some(s),
            None => self.partial_matches.next(),
        };

        match (current, current_scratch) {
            // Both Db and Scratch have value
            (Some(Ok(current)), Some(scratch)) => {
                if current.0 < &scratch.0[..] {
                    // Different key, db first, keep the scratch
                    self.current_scratch = Some(scratch);
                    Some(Ok((current.0.to_vec(), current.1)))
                } else {
                    // Different key, scratch first, keep the db
                    self.current = Some(Ok(current));
                    Some(Ok((scratch.0.to_vec(), scratch.1)))
                }
            }
            // Scratch is empty return db
            (Some(Ok(current)), None) => Some(Ok((current.0.to_vec(), current.1))),
            // Db is empty return scratch
            (None, Some(scratch)) => Some(Ok((scratch.0.to_vec(), scratch.1))),
            (Some(Err(e)), scratch) => {
                self.current_scratch = scratch;
                Some(Err(e))
            }
            (None, None) => None,
        }
    }
}

pub struct SingleIter<'env, 'a, V> {
    scratch: &'a BTreeMap<Vec<u8>, Op<V>>,
    iter: SingleIterRaw<'env, V>,
}

impl<'env, 'a, V> SingleIter<'env, 'a, V> {
    fn new(scratch: &'a BTreeMap<Vec<u8>, Op<V>>, iter: SingleIterRaw<'env, V>) -> Self {
        Self { scratch, iter }
    }
}

impl<'env, 'a, V> Iterator for SingleIter<'env, 'a, V>
where
    V: BufVal,
{
    type Item = Result<(&'env [u8], V), DatabaseError>;
    fn next(&mut self) -> Option<Self::Item> {
        use Op::*;
        match Iterator::next(&mut self.iter) {
            Some(Ok((k, v))) => Ok(match self.scratch.get(k) {
                Some(Put(scratch_val)) => Some((k, *scratch_val.clone())),
                Some(Delete) => None,
                None => Some((k, v)),
            })
            .transpose(),
            r => r,
        }
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
impl<'env, V> Iterator for SingleIterRaw<'env, V>
where
    V: BufVal,
{
    type Item = Result<(&'env [u8], V), DatabaseError>;

    fn next(&mut self) -> Option<Self::Item> {
        match self.0.next() {
            Some(Ok((k, Some(rkv::Value::Blob(buf))))) => Some(Ok((
                k,
                rmp_serde::from_read_ref(buf).expect(
                    "Failed to deserialize data from database. Database might be corrupted",
                ),
            ))),
            None => None,
            // TODO: Should this panic aswell?
            Some(Ok(_)) => Some(Err(DatabaseError::InvalidValue)),
            // This could be a IO error so returning it makes sense
            Some(Err(e)) => Some(Err(DatabaseError::from(e))),
        }
    }
}

impl<'env, 'a, V> FallibleIterator for SingleIter<'env, 'a, V>
where
    V: BufVal,
{
    type Item = (&'env [u8], V);
    type Error = DatabaseError;
    fn next(&mut self) -> Result<Option<Self::Item>, Self::Error> {
        Iterator::next(self).transpose()
    }
}

impl<'env, V> FallibleIterator for SingleIterRaw<'env, V>
where
    V: BufVal,
{
    type Item = (&'env [u8], V);
    type Error = DatabaseError;
    fn next(&mut self) -> Result<Option<Self::Item>, Self::Error> {
        Iterator::next(self).transpose()
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
    use std::collections::BTreeMap;

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

            let forward: Vec<_> = buf
                .iter_raw()?
                .map(Result::unwrap)
                .map(|(_, v)| v)
                .collect();
            let reverse: Vec<_> = buf
                .iter_raw_reverse()?
                .map(Result::unwrap)
                .map(|(_, v)| v)
                .collect();

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

            let forward = buf
                .iter_raw()
                .unwrap()
                .collect::<Result<Vec<_>, _>>()
                .unwrap();
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

            let forward: Vec<_> = buf.iter_raw().unwrap().collect::<Result<_, _>>().unwrap();
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
            let results = iter.collect::<Result<Vec<_>, _>>().unwrap();
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
}
