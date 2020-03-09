use super::{BufKey, BufVal, BufferedStore};
use crate::{
    error::{DatabaseError, DatabaseResult},
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
    pub fn new(reader: &'env R, db: SingleStore) -> DatabaseResult<Self> {
        Ok(Self {
            db,
            reader,
            scratch: HashMap::new(),
        })
    }

    pub fn get(&self, k: &K) -> DatabaseResult<Option<V>> {
        use Op::*;
        let val = match self.scratch.get(k) {
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

    // FIXME, This should not be public but we need a way to verify database in integration tests
    /// Fetch data from DB, deserialize into V type
    pub fn get_persisted(&self, k: &K) -> DatabaseResult<Option<V>> {
        match self.db.get(self.reader, k)? {
            Some(rkv::Value::Blob(buf)) => Ok(Some(bincode::deserialize(buf)?)),
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
