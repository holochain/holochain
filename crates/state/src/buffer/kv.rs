use super::{BufKey, BufVal, BufferedStore};
use crate::{
    error::{WorkspaceError, WorkspaceResult},
    prelude::{Readable, Reader, Writer},
};
use rkv::SingleStore;
use std::collections::HashMap;

/// Transactional operations on a KV store
/// Add: add this KV if the key does not yet exist
/// Mod: set the key to this value regardless of whether or not it already exists
/// Del: remove the KV
enum Op<V> {
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
    pub fn new(reader: &'env R, db: SingleStore) -> WorkspaceResult<Self> {
        Ok(Self {
            db,
            reader,
            scratch: HashMap::new(),
        })
    }

    pub fn get(&self, k: &K) -> WorkspaceResult<Option<V>> {
        use Op::*;
        let val = match self.scratch.get(k) {
            Some(Put(scratch_val)) => Some(*scratch_val.clone()),
            Some(Del) => None,
            None => self.get_persisted(k)?,
        };
        Ok(val)
    }

    pub fn put(&mut self, k: K, v: V) {
        // TODO, maybe give indication of whether the value existed or not
        let _ = self.scratch.insert(k, Op::Put(Box::new(v)));
    }

    pub fn delete(&mut self, k: K) {
        // TODO, maybe give indication of whether the value existed or not
        let _ = self.scratch.insert(k, Op::Del);
    }

    /// Fetch data from DB, deserialize into V type
    fn get_persisted(&self, k: &K) -> WorkspaceResult<Option<V>> {
        match self.db.get(self.reader, k)? {
            Some(rkv::Value::Blob(buf)) => Ok(Some(bincode::deserialize(buf)?)),
            None => Ok(None),
            Some(_) => Err(WorkspaceError::InvalidValue),
        }
    }

    /// Iterate over the underlying persisted data, NOT taking the scratch space into consideration
    pub fn iter_raw(&self) -> WorkspaceResult<SingleIter<V>> {
        Ok(SingleIter::new(self.db.iter_start(self.reader)?))
    }

    /// Iterate over the underlying persisted data in reverse, NOT taking the scratch space into consideration
    pub fn iter_raw_reverse(&self) -> WorkspaceResult<SingleIter<V>> {
        Ok(SingleIter::new(self.db.iter_end(self.reader)?))
    }
}

impl<'env, K, V, R> BufferedStore<'env> for KvBuf<'env, K, V, R>
where
    K: BufKey,
    V: BufVal,
    R: Readable,
{
    type Error = WorkspaceError;

    fn flush_to_txn(self, writer: &'env mut Writer) -> WorkspaceResult<()> {
        use Op::*;
        for (k, op) in self.scratch.iter() {
            match op {
                Put(v) => {
                    let buf = bincode::serialize(v)?;
                    let encoded = rkv::Value::Blob(&buf);
                    self.db.put(writer, k, &encoded)?;
                }
                Del => self.db.delete(writer, k)?,
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
/// TODO: Use FallibleIterator to prevent panics within iteration
impl<'env, V> Iterator for SingleIter<'env, V>
where
    V: BufVal,
{
    type Item = (&'env [u8], V);

    fn next(&mut self) -> Option<Self::Item> {
        match self.0.next() {
            Some(Ok((k, Some(rkv::Value::Blob(buf))))) => Some((
                k,
                bincode::deserialize(buf).expect("Failed to deserialize value"),
            )),
            None => None,
            x => {
                dbg!(x);
                panic!("TODO");
            }
        }
    }
}

#[cfg(test)]
pub mod tests {

    use super::{KvBuf, BufferedStore};
    use crate::{
        env::{ReadManager, WriteManager},
        error::{WorkspaceError, WorkspaceResult},
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

    type TestBuf<'a> = KvBuf<'a, &'a str, V>;

    #[test]
    fn kv_iterators() -> WorkspaceResult<()> {
        let env = test_env();
        let db = env.inner().open_single("kv", StoreOptions::create())?;

        env.with_reader::<WorkspaceError, _, _>(|reader| {
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

    #[test]
    fn kv_empty_iterators() -> WorkspaceResult<()> {
        let env = test_env();
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
    #[test]
    fn kv_store_sanity_check() -> WorkspaceResult<()> {
        let env = test_env();
        let db1 = env.inner().open_single("kv1", StoreOptions::create())?;
        let db2 = env.inner().open_single("kv1", StoreOptions::create())?;
        let mut writer = env.writer()?;

        let testval = TestVal {
            name: "Joe".to_owned(),
        };

        env.with_reader::<WorkspaceError, _, _>(|reader| {
            let mut kv1: KvBuf<String, TestVal> = KvBuf::new(&reader, db1)?;
            let mut kv2: KvBuf<String, String> = KvBuf::new(&reader, db2)?;

            kv1.put("hi".to_owned(), testval.clone());
            kv2.put("salutations".to_owned(), "folks".to_owned());

            // Check that the underlying store contains no changes yet
            assert_eq!(kv1.get_persisted(&"hi".to_owned())?, None);
            assert_eq!(kv2.get_persisted(&"salutations".to_owned())?, None);
            kv1.flush_to_txn(&mut writer)?;

            // Ensure that mid-transaction, there has still been no persistence,
            // just for kicks
            let kv1a: KvBuf<String, TestVal> = KvBuf::new(&reader, db1)?;
            assert_eq!(kv1a.get_persisted(&"hi".to_owned())?, None);
            kv2.flush_to_txn(&mut writer)?;
            Ok(())
        })?;

        // Finish finalizing the transaction
        writer.commit()?;

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
}
