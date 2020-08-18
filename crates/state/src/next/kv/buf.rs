use super::check_empty_key;
use super::{BufKey, BufVal, BufferedStore, KvStore};
use crate::env::ReadManager;
use crate::{
    env::EnvironmentRead,
    error::{DatabaseError, DatabaseResult},
    prelude::{Readable, Writer},
};
use rkv::SingleStore;
use std::collections::BTreeMap;

#[derive(shrinkwraprs::Shrinkwrap, derive_more::From)]
#[shrinkwrap(mutable)]
pub struct Scratch<K, V>(pub BTreeMap<K, Op<V>>);

impl<K: BufKey, V: BufVal> Scratch<K, V> {
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
pub struct KvBuf<K, V>
where
    K: BufKey,
    V: BufVal,
{
    env: EnvironmentRead,
    db: SingleStore,
    scratch: Scratch<K, V>,
}

impl<'env, K, V> KvBuf<K, V>
where
    K: BufKey,
    V: BufVal,
{
    /// Create a new KvBuf from a read-only transaction and a database reference
    pub fn new(env: EnvironmentRead, db: SingleStore) -> DatabaseResult<Self> {
        Ok(Self {
            env,
            db,
            scratch: BTreeMap::new().into(),
        })
    }

    pub fn store(&self) -> KvStore<K, V> {
        KvStore::new(self.db)
    }

    /// See if a value exists, avoiding deserialization
    pub fn contains_used<R: Readable>(&self, r: &R, k: &K) -> DatabaseResult<bool> {
        check_empty_key(k)?;
        use Op::*;
        let exists = match self.scratch.get(k) {
            Some(Put(_)) => true,
            Some(Delete) => false,
            None => self.store().get(r, k)?.is_some(),
        };
        Ok(exists)
    }

    /// See if a value exists, avoiding deserialization
    pub async fn contains_fresh(&self, k: &K) -> DatabaseResult<bool> {
        self.env
            .guard()
            .await
            .with_reader(|reader| self.contains_used(&reader, k))
    }

    /// Get a value, taking the scratch space into account,
    /// or from persistence if needed
    pub fn get_used<R: Readable>(&self, r: &R, k: &K) -> DatabaseResult<Option<V>> {
        check_empty_key(k)?;
        use Op::*;
        let val = match self.scratch.get(k) {
            Some(Put(scratch_val)) => Some(*scratch_val.clone()),
            Some(Delete) => None,
            None => self.store().get(r, k)?,
        };
        Ok(val)
    }

    /// Get a value, taking the scratch space into account,
    /// or from persistence if needed
    pub async fn get_fresh<R: Readable>(&self, k: &K) -> DatabaseResult<Option<V>> {
        self.env
            .guard()
            .await
            .with_reader(|reader| self.get_used(&reader, k))
    }

    /// Update the scratch space to record a Put operation for the KV
    pub fn put(&mut self, k: K, v: V) -> DatabaseResult<()> {
        check_empty_key(&k)?;
        self.scratch.insert(k, Op::Put(Box::new(v)));
        Ok(())
    }

    /// Update the scratch space to record a Delete operation for the KV
    pub fn delete(&mut self, k: K) -> DatabaseResult<()> {
        check_empty_key(&k)?;
        self.scratch.insert(k, Op::Delete);
        Ok(())
    }

    #[cfg(test)]
    pub(crate) fn scratch(&self) -> &Scratch<K, V> {
        &self.scratch
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
    //     check_empty_key(&k)?;

    //     let key = k.as_ref().to_vec();
    //     Ok(SingleKeyIter::new(
    //         SingleFromIter::new(&self.scratch, self.iter_raw_from(k)?, key.clone()),
    //         key,
    //     ))
    // }

    // /// Iterate from a key onwards
    // pub fn iter_from(&self, k: K) -> DatabaseResult<SingleFromIter<V>> {
    //     check_empty_key(&k)?;

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

    // TODO: This should be cfg test but can't because it's in a different crate
    /// Clear all scratch and db, useful for tests
    pub fn clear_all(&mut self, writer: &mut Writer) -> DatabaseResult<()> {
        self.scratch.clear();
        Ok(self.db.clear(writer)?)
    }
}

impl<K, V> BufferedStore for KvBuf<K, V>
where
    K: BufKey,
    V: BufVal,
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

        for (k, op) in self.scratch.iter() {
            match op {
                Put(v) => {
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

        Ok(())
    }
}

/////////////////////////////////
