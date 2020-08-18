use crate::buffer::{BufKey, BufVal, BufferedStore};
use crate::{
    error::{DatabaseError, DatabaseResult},
    prelude::{Readable, Reader, Writer},
};
use fallible_iterator::{DoubleEndedFallibleIterator, FallibleIterator};
use rkv::{SingleStore, StoreError};
use std::{collections::BTreeMap, marker::PhantomData};
use tracing::*;

#[cfg(test)]
mod test;

#[derive(shrinkwraprs::Shrinkwrap, derive_more::From)]
#[shrinkwrap(mutable)]
pub struct Scratch<V>(pub BTreeMap<Vec<u8>, Op<V>>);

impl<V> Scratch<V> {
    pub fn new() -> Self {
        Self(BTreeMap::new())
    }
}

/// Transactional operations on a KV store
/// Put: add or replace this KV
/// Delete: remove the KV
#[derive(Clone, Debug, PartialEq)]
pub enum Op<V> {
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
    scratch: &'env mut Scratch<V>,
    /// Key needs to be the same type for each KvBuf
    __key: PhantomData<K>,
}

impl<'env, K, V, R> KvBuf<'env, K, V, R>
where
    K: BufKey,
    V: BufVal,
    R: Readable,
{
    /// Create a new KvBuf from a read-only transaction and a database reference
    pub fn new(
        reader: &'env R,
        db: SingleStore,
        scratch: &'env mut Scratch<V>,
    ) -> DatabaseResult<Self> {
        Ok(Self {
            db,
            reader,
            scratch,
            __key: PhantomData,
        })
    }

    /// See if a value exists, avoiding deserialization
    pub fn contains(&self, k: &K) -> DatabaseResult<bool> {
        Self::empty_key(&k)?;
        use Op::*;
        let exists = match self.scratch.get(k.as_ref()) {
            Some(Put(_)) => true,
            Some(Delete) => false,
            None => self.db.get(self.reader, k)?.is_some(),
        };
        Ok(exists)
    }

    /// Get a value, taking the scratch space into account,
    /// or from persistence if needed
    pub fn get(&self, k: &K) -> DatabaseResult<Option<V>> {
        Self::empty_key(&k)?;
        use Op::*;
        let val = match self.scratch.get(k.as_ref()) {
            Some(Put(scratch_val)) => Some(*scratch_val.clone()),
            Some(Delete) => None,
            None => self.get_persisted(k)?,
        };
        Ok(val)
    }

    /// Update the scratch space to record a Put operation for the KV
    pub fn put(&mut self, k: K, v: V) -> DatabaseResult<()> {
        Self::empty_key(&k)?;
        let k = k.as_ref().to_vec();
        self.scratch.insert(k, Op::Put(Box::new(v)));
        Ok(())
    }

    /// Update the scratch space to record a Delete operation for the KV
    pub fn delete(&mut self, k: K) -> DatabaseResult<()> {
        Self::empty_key(&k)?;
        let k = k.as_ref().to_vec();
        self.scratch.insert(k, Op::Delete);
        Ok(())
    }

    /// Fetch data from DB, deserialize into V type
    fn get_persisted(&self, k: &K) -> DatabaseResult<Option<V>> {
        Self::empty_key(&k)?;
        match self.db.get(self.reader, k)? {
            Some(rkv::Value::Blob(buf)) => Ok(Some(holochain_serialized_bytes::decode(buf)?)),
            None => Ok(None),
            Some(_) => Err(DatabaseError::InvalidValue),
        }
    }

    #[cfg(test)]
    pub(crate) fn scratch(&self) -> &Scratch<V> {
        self.scratch
    }

    // /// Iterator that checks the scratch space
    // pub fn iter(&self) -> DatabaseResult<SingleIter<V>> {
    //     Ok(SingleIter::new(
    //         &self.scratch,
    //         self.scratch.iter(),
    //         self.iter_raw()?,
    //     ))
    // }

    // /// Iterator that tracks elements so they can be deleted
    // pub fn drain_iter(&mut self) -> DatabaseResult<DrainIter<V>> {
    //     Ok(DrainIter::new(
    //         &mut self.scratch,
    //         SingleIterRaw::new(
    //             self.db.iter_start(self.reader)?,
    //             self.db.iter_end(self.reader)?,
    //         ),
    //     ))
    // }

    // /// Iterator that tracks elements so they can be deleted but in reverse
    // pub fn drain_iter_reverse(&mut self) -> DatabaseResult<fallible_iterator::Rev<DrainIter<V>>> {
    //     Ok(DrainIter::new(
    //         &mut self.scratch,
    //         SingleIterRaw::new(
    //             self.db.iter_start(self.reader)?,
    //             self.db.iter_end(self.reader)?,
    //         ),
    //     )
    //     .rev())
    // }

    // /// Iterator that returns all partial matches to this key
    // pub fn iter_all_key_matches(&self, k: K) -> DatabaseResult<SingleKeyIter<V>> {
    //     Self::empty_key(&k)?;

    //     let key = k.as_ref().to_vec();
    //     Ok(SingleKeyIter::new(
    //         SingleFromIter::new(&self.scratch, self.iter_raw_from(k)?, key.clone()),
    //         key,
    //     ))
    // }

    // /// Iterate from a key onwards
    // pub fn iter_from(&self, k: K) -> DatabaseResult<SingleFromIter<V>> {
    //     Self::empty_key(&k)?;

    //     let key = k.as_ref().to_vec();
    //     Ok(SingleFromIter::new(
    //         &self.scratch,
    //         self.iter_raw_from(k)?,
    //         key,
    //     ))
    // }

    // /// Iterate over the data in reverse
    // pub fn iter_reverse(&self) -> DatabaseResult<fallible_iterator::Rev<SingleIter<V>>> {
    //     Ok(SingleIter::new(&self.scratch, self.scratch.iter(), self.iter_raw()?).rev())
    // }

    // /// Iterate over the underlying persisted data, NOT taking the scratch space into consideration
    // pub fn iter_raw(&self) -> DatabaseResult<SingleIterRaw<V>> {
    //     Ok(SingleIterRaw::new(
    //         self.db.iter_start(self.reader)?,
    //         self.db.iter_end(self.reader)?,
    //     ))
    // }

    // /// Iterate from a key onwards without scratch space
    // pub fn iter_raw_from(&self, k: K) -> DatabaseResult<SingleIterRaw<V>> {
    //     Ok(SingleIterRaw::new(
    //         self.db.iter_from(self.reader, k)?,
    //         self.db.iter_end(self.reader)?,
    //     ))
    // }

    // /// Iterate over the underlying persisted data in reverse, NOT taking the scratch space into consideration
    // pub fn iter_raw_reverse(&self) -> DatabaseResult<fallible_iterator::Rev<SingleIterRaw<V>>> {
    //     Ok(SingleIterRaw::new(
    //         self.db.iter_start(self.reader)?,
    //         self.db.iter_end(self.reader)?,
    //     )
    //     .rev())
    // }

    // Empty keys break lmdb
    fn empty_key(k: &K) -> DatabaseResult<()> {
        if k.as_ref().is_empty() {
            Err(DatabaseError::EmptyKey)
        } else {
            Ok(())
        }
    }

    // TODO: This should be cfg test but can't because it's in a different crate
    /// Clear all scratch and db, useful for tests
    pub fn clear_all(&mut self, writer: &mut Writer) -> DatabaseResult<()> {
        self.scratch.clear();
        Ok(self.db.clear(writer)?)
    }
}

impl<'env, K, V, R> BufferedStore<'env> for KvBuf<'env, K, V, R>
where
    K: BufKey,
    V: BufVal,
    R: Readable,
{
    type Error = DatabaseError;

    fn is_clean(&self) -> bool {
        self.scratch.is_empty()
    }

    fn flush_to_txn(self, writer: &'env mut Writer) -> DatabaseResult<()> {
        use Op::*;
        if self.is_clean() {
            return Ok(());
        }

        for (k, op) in self.scratch.iter() {
            match op {
                Put(v) => {
                    // TODO: consider using a more explicit msgpack encoding,
                    // with more data and less chance of "slippage"
                    // let mut buf = Vec::with_capacity(128);
                    // let mut se = rmp_serde::encode::Serializer::new(buf)
                    //     .with_struct_map()
                    //     .with_string_variants();
                    // v.serialize(&mut se)?;
                    let buf = holochain_serialized_bytes::encode(v)?;
                    let encoded = rkv::Value::Blob(&buf);
                    self.db.put(writer, k, &encoded)?;
                }
                Delete => match self.db.delete(writer, k) {
                    Err(rkv::StoreError::LmdbError(rkv::LmdbError::NotFound)) => (),
                    r => r?,
                },
            }
        }

        // TODO: make sure all next-bufs do this!
        self.scratch.clear();

        Ok(())
    }
}

/////////////////////////////////
