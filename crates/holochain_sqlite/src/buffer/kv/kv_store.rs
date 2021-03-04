use super::KvStoreT;
use crate::buffer::check_empty_key;
use crate::buffer::iter::SingleIterRaw;
use crate::error::DatabaseError;
use crate::error::DatabaseResult;
use crate::prelude::*;
use fallible_iterator::FallibleIterator;
use SingleTable;

/// Wrapper around an rkv SingleTable which provides strongly typed values
pub struct KvStore<K, V>
where
    K: BufKey,
    V: BufVal,
{
    table: SingleTable,
    __phantom: std::marker::PhantomData<(K, V)>,
}

impl<K, V> KvStoreT<K, V> for KvStore<K, V>
where
    K: BufKey,
    V: BufVal,
{
    /// Fetch data from DB as raw byte slice
    fn get_bytes<'env, R: Readable>(
        &'env self,
        reader: &'env mut R,
        k: &K,
    ) -> DatabaseResult<Option<Vec<u8>>> {
        check_empty_key(k)?;
        match self.table.get(reader, k)? {
            Some(rusqlite::types::Value::Blob(buf)) => Ok(Some(buf)),
            None => Ok(None),
            Some(_) => Err(DatabaseError::InvalidValue),
        }
    }

    /// Fetch data from DB, deserialize into V type
    fn get<R: Readable>(&self, reader: &mut R, k: &K) -> DatabaseResult<Option<V>> {
        check_empty_key(k)?;
        match self.get_bytes(reader, k)? {
            Some(bytes) => Ok(Some(holochain_serialized_bytes::decode(&bytes)?)),
            None => Ok(None),
        }
    }

    /// Put V into DB as serialized data
    fn put(&self, writer: &mut Writer, k: &K, v: &V) -> DatabaseResult<()> {
        let buf = holochain_serialized_bytes::encode(v)?;
        let encoded = rusqlite::types::Value::Blob(buf);
        self.table.put(writer, k, &encoded)?;
        Ok(())
    }

    /// Delete value from DB
    fn delete(&self, writer: &mut Writer, k: &K) -> DatabaseResult<()> {
        Ok(self.table.delete(writer, k)?)
    }

    /// Iterate over the underlying persisted data
    fn iter<'env, R: Readable>(
        &self,
        reader: &'env mut R,
    ) -> DatabaseResult<SingleIterRaw<'env, V>> {
        todo!("lmdb iter")
        // Ok(SingleIterRaw::new(
        //     self.table.iter_start(reader)?,
        //     self.table.iter_end(reader)?,
        // ))
    }

    /// Iterate from a key onwards
    fn iter_from<'env, R: Readable>(
        &self,
        reader: &'env mut R,
        k: K,
    ) -> DatabaseResult<SingleIterRaw<'env, V>> {
        check_empty_key(&k)?;
        todo!("lmdb iter")
        // Ok(SingleIterRaw::new(
        //     self.table.iter_from(reader, k)?,
        //     self.table.iter_end(reader)?,
        // ))
    }

    /// Iterate over the underlying persisted data in reverse
    fn iter_reverse<'env, R: Readable>(
        &self,
        reader: &'env mut R,
    ) -> DatabaseResult<fallible_iterator::Rev<SingleIterRaw<'env, V>>> {
        todo!("lmdb iter")
        // Ok(SingleIterRaw::new(self.table.iter_start(reader)?, self.table.iter_end(reader)?).rev())
    }
}

impl<K, V> KvStore<K, V>
where
    K: BufKey,
    V: BufVal,
{
    /// Create a new KvStore
    pub fn new(table: SingleTable) -> Self {
        Self {
            table,
            __phantom: std::marker::PhantomData,
        }
    }

    /// Accessor for raw Rkv DB
    pub fn table(&self) -> SingleTable {
        self.table.clone()
    }

    // TODO: This should be cfg test but can't because it's in a different crate
    /// Clear table, useful for tests
    pub fn delete_all(&mut self, writer: &mut Writer) -> DatabaseResult<()> {
        Ok(self.table.clear(writer)?)
    }
}
