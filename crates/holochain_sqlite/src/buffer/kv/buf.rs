use super::KvIntStore;
use crate::buffer::check_empty_key;
use crate::buffer::iter::DrainIter;
use crate::buffer::iter::SingleIter;
use crate::buffer::iter::SingleIterFrom;
use crate::buffer::iter::SingleIterKeyMatch;
use crate::buffer::kv::generic::KvStoreT;
use crate::buffer::kv::KvStore;
use crate::buffer::BufferedStore;
use crate::db::DbRead;
use crate::error::DatabaseError;
use crate::error::DatabaseResult;
use crate::fresh_reader;
use crate::prelude::*;
use fallible_iterator::FallibleIterator;
use std::collections::BTreeMap;

#[cfg(test)]
mod iter_tests;
#[cfg(test)]
mod tests;

pub type KvBufUsed<K, V> = Used<K, V, KvStore<K, V>>;
pub type KvBufFresh<K, V> = Fresh<K, V, KvStore<K, V>>;
pub type KvIntBufUsed<V> = Used<IntKey, V, KvIntStore<V>>;
pub type KvIntBufFresh<V> = Fresh<IntKey, V, KvIntStore<V>>;

type Scratch<V> = BTreeMap<Vec<u8>, KvOp<V>>;

/// Transactional operations on a KV store
#[derive(Clone, Debug, PartialEq)]
pub enum KvOp<V> {
    /// add or replace the value at a key
    Put(Box<V>),
    /// remove the value at a key
    Delete,
}

pub struct Used<K, V, Store>
where
    K: BufKey,
    V: BufVal,
    Store: KvStoreT<K, V>,
{
    store: Store,
    scratch: Scratch<V>,
    __phantom: std::marker::PhantomData<K>,
}

impl<'env, V> Used<IntKey, V, KvIntStore<V>>
where
    V: BufVal,
{
    /// Constructor
    // FIXME: why does this conflict with the other `new` when it's called just "new"?
    pub fn new_int(db: IntegerTable) -> Self {
        Self {
            store: KvIntStore::new(db),
            scratch: BTreeMap::new(),
            __phantom: std::marker::PhantomData,
        }
    }

    // TODO: This should be cfg test but can't because it's in a different crate
    /// Clear all scratch and db, useful for tests
    pub fn clear_all(&mut self, writer: &mut Writer) -> DatabaseResult<()> {
        self.scratch.clear();
        Ok(self.store.delete_all(writer)?)
    }
}

impl<'env, K, V> Used<K, V, KvStore<K, V>>
where
    K: BufKey,
    V: BufVal,
{
    /// Constructor
    pub fn new(db: SingleTable) -> Self {
        Self {
            store: KvStore::new(db),
            scratch: BTreeMap::new(),
            __phantom: std::marker::PhantomData,
        }
    }

    // TODO: This should be cfg test but can't because it's in a different crate
    /// Clear all scratch and db, useful for tests
    pub fn clear_all(&mut self, writer: &mut Writer) -> DatabaseResult<()> {
        self.scratch.clear();
        Ok(self.store.delete_all(writer)?)
    }
}

impl<'env, K, V, Store> Used<K, V, Store>
where
    K: BufKey,
    V: BufVal,
    Store: KvStoreT<K, V>,
{
    pub fn store(&self) -> &Store {
        &self.store
    }

    /// See if a value exists, avoiding deserialization
    pub fn contains<R: Readable>(&self, r: &mut R, k: &K) -> DatabaseResult<bool> {
        check_empty_key(k)?;
        use KvOp::*;
        let exists = match self.scratch.get(k.as_ref()) {
            Some(Put(_)) => true,
            Some(Delete) => false,
            None => self.store.get(r, k)?.is_some(),
        };
        Ok(exists)
    }

    /// Check if a value is in the scratch space
    pub fn contains_in_scratch(&self, k: &K) -> DatabaseResult<bool> {
        check_empty_key(k)?;
        use KvOp::*;
        let exists = match self.scratch.get(k.as_ref()) {
            Some(Put(_)) => true,
            Some(Delete) => false,
            None => false,
        };
        Ok(exists)
    }

    /// Get a value, taking the scratch space into account,
    /// or from persistence if needed
    pub fn get<R: Readable>(&self, r: &mut R, k: &K) -> DatabaseResult<Option<V>> {
        check_empty_key(k)?;
        use KvOp::*;
        let val = match self.scratch.get(k.as_ref()) {
            Some(Put(scratch_val)) => Some(*scratch_val.clone()),
            Some(Delete) => None,
            None => self.store.get(r, k)?,
        };
        Ok(val)
    }

    /// Update the scratch space to record a Put operation for the KV
    pub fn put(&mut self, k: K, v: V) -> DatabaseResult<()> {
        check_empty_key(&k)?;
        self.scratch
            .insert(k.to_key_bytes(), KvOp::Put(Box::new(v)));
        Ok(())
    }

    /// Update the scratch space to record a Delete operation for the KV
    pub fn delete(&mut self, k: K) -> DatabaseResult<()> {
        check_empty_key(&k)?;
        self.scratch.insert(k.to_key_bytes(), KvOp::Delete);
        Ok(())
    }

    /// Update the scratch space to remove a Delete operation for the KV
    pub fn cancel_delete(&mut self, k: K) -> DatabaseResult<()> {
        check_empty_key(&k)?;
        let k = k.to_key_bytes();
        if let Some(&KvOp::Delete) = self.scratch.get(&k) {
            self.scratch.remove(&k);
        }
        Ok(())
    }

    pub fn is_scratch_fresh(&self) -> bool {
        self.scratch.is_empty()
    }

    #[cfg(test)]
    pub(crate) fn scratch(&self) -> &Scratch<V> {
        &self.scratch
    }

    /// Iterator that checks the scratch space
    pub fn iter<'r, R: Readable>(&'r self, r: &'r mut R) -> DatabaseResult<SingleIter<'_, V>> {
        Ok(SingleIter::new(
            &self.scratch,
            self.scratch.iter(),
            self.store.iter(r)?,
        ))
    }

    /// Iterator that tracks elements so they can be deleted
    pub fn drain_iter<'r, R: Readable>(
        &mut self,
        r: &'r mut R,
    ) -> DatabaseResult<DrainIter<'_, V>> {
        Ok(DrainIter::new(&mut self.scratch, self.store.iter(r)?))
    }

    /// Iterator that tracks elements so they can be deleted.
    /// This allows filtering before the deletes are added
    ///
    /// NB: if we ever have to implement other iter methods, we should
    /// consider passing in a raw iter to DrainIter
    pub fn drain_iter_filter<'r, F, R>(
        &'r mut self, // maybe need new lifetime instead of 'r
        r: &'r mut R,
        filter: F,
    ) -> DatabaseResult<DrainIter<'_, V>>
    where
        F: FnMut(&(Vec<u8>, V)) -> Result<bool, DatabaseError> + 'r,
        R: Readable,
    {
        Ok(DrainIter::new(
            &mut self.scratch,
            self.store.iter(r)?.filter(filter),
        ))
    }

    /// Iterator that returns all partial matches to this key
    pub fn iter_all_key_matches<'r, R: Readable>(
        &'r self,
        r: &'r mut R,
        k: K,
    ) -> DatabaseResult<SingleIterKeyMatch<'r, V>> {
        check_empty_key(&k)?;
        let key = k.as_ref().to_vec();
        Ok(SingleIterKeyMatch::new(
            SingleIterFrom::new(&self.scratch, self.store.iter_from(r, k)?, key.clone()),
            key,
        ))
    }

    /// Iterate from a key onwards
    pub fn iter_from<'r, R: Readable>(
        &'r self,
        r: &'r mut R,
        k: K,
    ) -> DatabaseResult<SingleIterFrom<'_, V>> {
        check_empty_key(&k)?;

        let key = k.as_ref().to_vec();
        Ok(SingleIterFrom::new(
            &self.scratch,
            self.store.iter_from(r, k)?,
            key,
        ))
    }

    /// Iterate over the data in reverse
    #[deprecated = "just use rev()"]
    pub fn iter_reverse<'r, R: Readable>(
        &'r self,
        r: &'r mut R,
    ) -> DatabaseResult<fallible_iterator::Rev<SingleIter<'_, V>>> {
        Ok(self.iter(r)?.rev())
    }

    /// Iterator that tracks elements so they can be deleted but in reverse
    #[deprecated = "just use rev()"]
    pub fn drain_iter_reverse<'r, R: Readable>(
        &'r mut self,
        r: &'r mut R,
    ) -> DatabaseResult<fallible_iterator::Rev<DrainIter<'_, V>>> {
        Ok(self.drain_iter(r)?.rev())
    }
}

#[derive(shrinkwraprs::Shrinkwrap)]
#[shrinkwrap(mutable, unsafe_ignore_visibility)]
pub struct Fresh<K, V, Store>
where
    K: BufKey,
    V: BufVal,
    Store: KvStoreT<K, V>,
{
    env: DbRead,
    #[shrinkwrap(main_field)]
    inner: Used<K, V, Store>,
}

impl<K, V> Fresh<K, V, KvStore<K, V>>
where
    K: BufKey,
    V: BufVal,
{
    /// Create a new Fresh
    pub fn new(env: DbRead, db: SingleTable) -> Self {
        Self {
            env,
            inner: Used::new(db),
        }
    }
}

impl<V> Fresh<IntKey, V, KvIntStore<V>>
where
    V: BufVal,
{
    /// Create a new Fresh
    pub fn new(env: DbRead, db: IntegerTable) -> Self {
        Self {
            env,
            inner: Used::new_int(db),
        }
    }
}

impl<K, V, Store> Fresh<K, V, Store>
where
    K: BufKey,
    V: BufVal,
    Store: KvStoreT<K, V>,
{
    pub fn env(&self) -> &DbRead {
        &self.env
    }

    /// See if a value exists, avoiding deserialization
    pub fn contains(&self, k: &K) -> DatabaseResult<bool> {
        fresh_reader!(self.env, |mut r| self.inner.contains(&mut r, k))
    }

    /// Get a value, taking the scratch space into account,
    /// or from persistence if needed
    pub fn get(&self, k: &K) -> DatabaseResult<Option<V>> {
        fresh_reader!(self.env, |mut r| self.inner.get(&mut r, k))
    }
}

impl<K, V> BufferedStore for KvBufUsed<K, V>
where
    K: BufKey,
    V: BufVal,
{
    type Error = DatabaseError;

    fn is_clean(&self) -> bool {
        self.scratch.is_empty()
    }

    fn flush_to_txn_ref(&mut self, writer: &mut Writer) -> DatabaseResult<()> {
        use KvOp::*;

        if self.is_clean() {
            return Ok(());
        }

        for (k, op) in self.scratch.iter() {
            match op {
                Put(v) => {
                    let buf = holochain_serialized_bytes::encode(v)?;
                    let encoded = rusqlite::types::Value::Blob(buf);
                    self.store.table().put(writer, k, &encoded)?;
                }
                Delete => self
                    .store
                    .table()
                    .delete(writer, k)
                    .or_else(DatabaseError::ok_if_not_found)?,
            }
        }

        Ok(())
    }
}

impl<V> BufferedStore for KvIntBufUsed<V>
where
    V: BufVal,
{
    type Error = DatabaseError;

    fn is_clean(&self) -> bool {
        self.scratch.is_empty()
    }

    fn flush_to_txn_ref(&mut self, writer: &mut Writer) -> DatabaseResult<()> {
        use KvOp::*;

        if self.is_clean() {
            return Ok(());
        }

        for (k, op) in self.scratch.iter() {
            match op {
                Put(v) => {
                    let buf = holochain_serialized_bytes::encode(v)?;
                    let encoded = rusqlite::types::Value::Blob(buf);
                    self.store.table().put(
                        writer,
                        IntKey::from_key_bytes_or_friendly_panic(&k).as_ref(),
                        &encoded,
                    )?;
                }
                Delete => self
                    .store
                    .table()
                    .delete(
                        writer,
                        IntKey::from_key_bytes_or_friendly_panic(&k).as_ref(),
                    )
                    .or_else(DatabaseError::ok_if_not_found)?,
            }
        }

        Ok(())
    }
}

impl<K, V> BufferedStore for KvBufFresh<K, V>
where
    K: BufKey,
    V: BufVal,
{
    type Error = DatabaseError;

    fn is_clean(&self) -> bool {
        self.scratch.is_empty()
    }

    fn flush_to_txn_ref(&mut self, writer: &mut Writer) -> DatabaseResult<()> {
        self.inner.flush_to_txn_ref(writer)
    }
}

impl<V> BufferedStore for KvIntBufFresh<V>
where
    V: BufVal,
{
    type Error = DatabaseError;

    fn is_clean(&self) -> bool {
        self.scratch.is_empty()
    }

    fn flush_to_txn_ref(&mut self, writer: &mut Writer) -> DatabaseResult<()> {
        self.inner.flush_to_txn_ref(writer)
    }
}

/// Create an Used with a clone of the scratch
/// from another Used
impl<'env, K, V> From<&Used<K, V, KvStore<K, V>>> for Used<K, V, KvStore<K, V>>
where
    K: BufKey,
    V: BufVal,
{
    fn from(other: &Used<K, V, KvStore<K, V>>) -> Self {
        Self {
            store: KvStore::new(other.store.table()),
            scratch: other.scratch.clone(),
            __phantom: std::marker::PhantomData,
        }
    }
}
