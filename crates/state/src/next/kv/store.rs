use super::{check_empty_key, iter::SingleIterRaw, BufKey, BufVal};
use crate::{
    error::{DatabaseError, DatabaseResult},
    prelude::*,
};
use fallible_iterator::FallibleIterator;
use rkv::SingleStore;

/// Wrapper around an rkv SingleStore which provides strongly typed values
pub struct Kv<K, V>
where
    K: BufKey,
    V: BufVal,
{
    db: SingleStore,
    __phantom: std::marker::PhantomData<(K, V)>,
}

impl<K, V> Kv<K, V>
where
    K: BufKey,
    V: BufVal,
{
    /// Create a new IntKvBuf from a read-only transaction and a database reference
    pub fn new(db: SingleStore) -> Self {
        Self {
            db,
            __phantom: std::marker::PhantomData,
        }
    }

    /// Fetch data from DB as raw byte slice
    pub fn get_bytes<'env, R: Readable>(
        &self,
        reader: &'env R,
        k: &K,
    ) -> DatabaseResult<Option<&'env [u8]>> {
        check_empty_key(k)?;
        match self.db.get(reader, k)? {
            Some(rkv::Value::Blob(buf)) => Ok(Some(buf)),
            None => Ok(None),
            Some(_) => Err(DatabaseError::InvalidValue),
        }
    }

    /// Fetch data from DB, deserialize into V type
    pub fn get<R: Readable>(&self, reader: &R, k: &K) -> DatabaseResult<Option<V>> {
        check_empty_key(k)?;
        match self.get_bytes(reader, k)? {
            Some(bytes) => Ok(Some(holochain_serialized_bytes::decode(bytes)?)),
            None => Ok(None),
        }
    }

    /// Put V into DB as serialized data
    pub fn put(&self, writer: &mut Writer, k: &K, v: &V) -> DatabaseResult<()> {
        check_empty_key(k)?;
        let buf = holochain_serialized_bytes::encode(v)?;
        let encoded = rkv::Value::Blob(&buf);
        self.db.put(writer, k, &encoded)?;
        Ok(())
    }

    /// Delete value from DB
    pub fn delete(&self, writer: &mut Writer, k: &K) -> DatabaseResult<()> {
        check_empty_key(k)?;
        Ok(self.db.delete(writer, k)?)
    }

    /// Iterate over the underlying persisted data
    pub fn iter<'env, R: Readable>(
        &self,
        reader: &'env R,
    ) -> DatabaseResult<SingleIterRaw<'env, V>> {
        Ok(SingleIterRaw::new(
            self.db.iter_start(reader)?,
            self.db.iter_end(reader)?,
        ))
    }

    /// Iterate from a key onwards
    pub fn iter_from<'env, R: Readable>(
        &self,
        reader: &'env R,
        k: K,
    ) -> DatabaseResult<SingleIterRaw<'env, V>> {
        check_empty_key(&k)?;
        Ok(SingleIterRaw::new(
            self.db.iter_from(reader, k)?,
            self.db.iter_end(reader)?,
        ))
    }

    /// Iterate over the underlying persisted data in reverse
    pub fn iter_reverse<'env, R: Readable>(
        &self,
        reader: &'env R,
    ) -> DatabaseResult<fallible_iterator::Rev<SingleIterRaw<'env, V>>> {
        Ok(SingleIterRaw::new(self.db.iter_start(reader)?, self.db.iter_end(reader)?).rev())
    }
}
