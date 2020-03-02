use super::{BufferKey, BufferVal, StoreBuffer};
use crate::error::{WorkspaceError, WorkspaceResult};
use rkv::{Reader, Rkv, SingleStore, StoreOptions, Writer};
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
pub struct KvBuffer<'env, K, V>
where
    K: BufferKey,
    V: BufferVal,
{
    db: SingleStore,
    reader: &'env Reader<'env>,
    scratch: HashMap<K, KvOp<V>>,
}

impl<'env, K, V> KvBuffer<'env, K, V>
where
    K: BufferKey,
    V: BufferVal,
{
    pub fn new(reader: &'env Reader<'env>, db: SingleStore) -> WorkspaceResult<Self> {
        Ok(Self {
            db,
            reader,
            scratch: HashMap::new(),
        })
    }

    pub fn get(&self, k: &K) -> WorkspaceResult<Option<V>> {
        use KvOp::*;
        let val = match self.scratch.get(k) {
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
    fn get_persisted(&self, k: &K) -> WorkspaceResult<Option<V>> {
        match self.db.get(self.reader, k)? {
            Some(rkv::Value::Blob(buf)) => Ok(Some(rmp_serde::from_read_ref(buf)?)),
            None => Ok(None),
            Some(_) => Err(WorkspaceError::InvalidValue),
        }
    }

    fn iter_raw(&self) -> WorkspaceResult<SingleStoreIterTyped<V>> {
        Ok((SingleStoreIterTyped::new(self.db.iter_start(self.reader)?)))
    }

    fn iter_raw_reverse(&self) -> WorkspaceResult<SingleStoreIterTyped<V>> {
        Ok((SingleStoreIterTyped::new(self.db.iter_end(self.reader)?)))
    }
}

impl<'env, K, V> StoreBuffer<'env> for KvBuffer<'env, K, V>
where
    K: BufferKey,
    V: BufferVal,
{
    fn finalize(self, writer: &'env mut Writer) -> WorkspaceResult<()> {
        use KvOp::*;
        for (k, op) in self.scratch.iter() {
            match op {
                Put(v) => {
                    let buf = rmp_serde::to_vec_named(v)?;
                    let encoded = rkv::Value::Blob(&buf);
                    self.db.put(writer, k, &encoded)?;
                }
                Del => self.db.delete(writer, k)?,
            }
        }
        Ok(())
    }
}

pub struct SingleStoreIterTyped<'env, V>(
    rkv::store::single::Iter<'env>,
    std::marker::PhantomData<V>,
);

impl<'env, V> SingleStoreIterTyped<'env, V> {
    pub fn new(iter: rkv::store::single::Iter<'env>) -> Self {
        Self(iter, std::marker::PhantomData)
    }
}

impl<'env, V> Iterator for SingleStoreIterTyped<'env, V>
where
    V: BufferVal,
{
    type Item = V;

    fn next(&mut self) -> Option<Self::Item> {
        match self.0.next() {
            Some(Ok((_k, Some(rkv::Value::Blob(buf))))) => {
                (
                    // k.into(),
                    rmp_serde::from_read_ref(buf).unwrap()
                )
            }
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

    use super::{KvBuffer, StoreBuffer};
    use crate::{
        db::{ReadManager, WriteManager},
        env::{create_lmdb_env},
        test_utils::test_env,
    };
    use rkv::StoreOptions;
    use serde_derive::{Deserialize, Serialize};
    use tempdir::TempDir;

    #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
    struct TestVal {
        name: String,
    }

    #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
    struct V(u32);

    type TestBuf<'a> = KvBuffer<'a, &'a str, V>;

    #[test]
    fn kv_iterators() {
        let arc = test_env();
        let env = arc.read().unwrap();
        let db = env.open_single("kv", StoreOptions::create()).unwrap();
        let rm = ReadManager::new(&env);
        let wm = WriteManager::new(&env);

        rm.with_reader(|reader| {
            let mut buf: TestBuf = KvBuffer::new(&reader, db).unwrap();

            buf.put("a", V(1));
            buf.put("b", V(2));
            buf.put("c", V(3));
            buf.put("d", V(4));
            buf.put("e", V(5));

            wm.with_writer(|mut writer| buf.finalize(&mut writer))
                .unwrap();
            Ok(())
        })
        .unwrap();

        rm.with_reader(|reader| {
            let buf: TestBuf = KvBuffer::new(&reader, db).unwrap();

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
        let db = env.open_single("kv", StoreOptions::create()).unwrap();
        let rm = ReadManager::new(&env);

        rm.with_reader(|reader| {
            let buf: TestBuf = KvBuffer::new(&reader, db).unwrap();

            let forward: Vec<_> = buf.iter_raw().unwrap().collect();
            let reverse: Vec<_> = buf.iter_raw_reverse().unwrap().collect();

            assert_eq!(forward, vec![]);
            assert_eq!(reverse, vec![]);
            Ok(())
        })
        .unwrap();
    }

    /// TODO break up into smaller tests
    #[test]
    fn kv_store_sanity_check() {
        let arc = test_env();
        let env = arc.read().unwrap();
        let db1 = env.open_single("kv1", StoreOptions::create()).unwrap();
        let db2 = env.open_single("kv1", StoreOptions::create()).unwrap();
        let rm = ReadManager::new(&env);
        let mut writer = env.write().unwrap();

        let testval = TestVal {
            name: "Joe".to_owned(),
        };

        rm.with_reader(|reader| {
            let mut kv1: KvBuffer<String, TestVal> = KvBuffer::new(&reader, db1).unwrap();
            let mut kv2: KvBuffer<String, String> = KvBuffer::new(&reader, db2).unwrap();

            kv1.put("hi".to_owned(), testval.clone());
            kv2.put("salutations".to_owned(), "folks".to_owned());

            // Check that the underlying store contains no changes yet
            assert_eq!(kv1.get_persisted(&"hi".to_owned()).unwrap(), None);
            assert_eq!(kv2.get_persisted(&"salutations".to_owned()).unwrap(), None);
            kv1.finalize(&mut writer).unwrap();

            // Ensure that mid-transaction, there has still been no persistence,
            // just for kicks
            let kv1a: KvBuffer<String, TestVal> = KvBuffer::new(&reader, db1).unwrap();
            assert_eq!(kv1a.get_persisted(&"hi".to_owned()).unwrap(), None);
            kv2.finalize(&mut writer).unwrap();
            Ok(())
        })
        .unwrap();

        // Finish finalizing the transaction
        writer.commit().unwrap();

        rm.with_reader(|reader| {
            // Now open some fresh Readers to see that our data was persisted
            let kv1b: KvBuffer<String, TestVal> = KvBuffer::new(&reader, db1).unwrap();
            let kv2b: KvBuffer<String, String> = KvBuffer::new(&reader, db2).unwrap();
            // Check that the underlying store contains no changes yet
            assert_eq!(kv1b.get_persisted(&"hi".to_owned()).unwrap(), Some(testval));
            assert_eq!(
                kv2b.get_persisted(&"salutations".to_owned()).unwrap(),
                Some("folks".to_owned())
            );
            Ok(())
        })
        .unwrap();
    }
}
