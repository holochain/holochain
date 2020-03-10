//! An interface to an LMDB key-value store, with integer keys
//! This is unfortunately pure copypasta from KvBuf, since Rust doesn't support specialization yet
//! TODO, find *some* way to DRY up the two

use super::{BufIntKey, BufVal, BufferedStore};
use crate::{
    error::{DatabaseError, DatabaseResult},
    prelude::{Readable, Reader, Writer},
};
use rkv::IntegerStore;

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
    pub fn new(reader: &'env R, db: IntegerStore<K>) -> DatabaseResult<Self> {
        Ok(Self {
            db,
            reader,
            scratch: HashMap::new(),
        })
    }

    pub fn with_reader<RR: Readable>(&self, reader: &'env RR) -> IntKvBuf<'env, K, V, RR> {
        IntKvBuf {
            db: self.db,
            reader,
            scratch: HashMap::new(),
        }
    }

    pub fn get(&self, k: K) -> DatabaseResult<Option<V>> {
        use Op::*;
        let val = match self.scratch.get(&k) {
            Some(Put(scratch_val)) => Some(*scratch_val.clone()),
            Some(Del) => None,
            None => self.get_persisted(k)?,
        };
        Ok(val)
    }

    pub fn put(&mut self, k: K, v: V) {
        // FIXME, maybe give indication of whether the value existed or not
        let _ = self.scratch.insert(k, Op::Put(Box::new(v)));
    }

    pub fn delete(&mut self, k: K) {
        // FIXME, maybe give indication of whether the value existed or not
        let _ = self.scratch.insert(k, Op::Del);
    }

    /// Fetch data from DB, deserialize into V type
    fn get_persisted(&self, k: K) -> DatabaseResult<Option<V>> {
        match self.db.get(self.reader, k)? {
            Some(rkv::Value::Blob(buf)) => Ok(Some(bincode::deserialize(buf)?)),
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
                    let buf = bincode::serialize(v)?;
                    let encoded = rkv::Value::Blob(&buf);
                    self.db.put(writer, *k, &encoded)?;
                }
                Del => self.db.delete(writer, *k)?,
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

    use super::{BufferedStore, IntKvBuf};
    use crate::{
        env::{ReadManager, WriteManager},
        error::DatabaseResult,
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

    type Store<'a> = IntKvBuf<'a, u32, V>;

    #[test]
    fn kv_iterators() -> DatabaseResult<()> {
        let env = test_env();
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

    #[test]
    fn kv_empty_iterators() -> DatabaseResult<()> {
        let env = test_env();
        let db = env
            .inner()
            .open_integer("kv", StoreOptions::create())
            .unwrap();

        env.with_reader(|reader| {
            let buf: Store = IntKvBuf::new(&reader, db).unwrap();

            let forward: Vec<_> = buf.iter_raw().unwrap().collect();
            let reverse: Vec<_> = buf.iter_raw_reverse().unwrap().collect();

            assert_eq!(forward, vec![]);
            assert_eq!(reverse, vec![]);
            Ok(())
        })
    }
}
