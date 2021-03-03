use crate::buffer::iter::SingleIterRaw;
use crate::buffer::kv::KvStoreT;
use crate::error::DatabaseError;
use crate::error::DatabaseResult;
use crate::prelude::*;
use fallible_iterator::FallibleIterator;

type K = IntKey;

/// Wrapper around an rkv IntegerTable which provides strongly typed values
pub struct KvIntStore<V>
where
    V: BufVal,
{
    db: IntegerTable,
    __phantom: std::marker::PhantomData<V>,
}

impl<V> KvStoreT<K, V> for KvIntStore<V>
where
    V: BufVal,
{
    /// Fetch data from DB as raw byte slice
    fn get_bytes<'env, R: Readable>(
        &'env self,
        reader: &'env R,
        k: &K,
    ) -> DatabaseResult<Option<&'env [u8]>> {
        match self.db.get(reader, (*k).as_ref())? {
            Some(rusqlite::types::Value::Blob(buf)) => Ok(Some(buf)),
            None => Ok(None),
            Some(_) => Err(DatabaseError::InvalidValue),
        }
    }

    /// Fetch data from DB, deserialize into V type
    fn get<R: Readable>(&self, reader: &R, k: &K) -> DatabaseResult<Option<V>> {
        match self.get_bytes(reader, k)? {
            Some(bytes) => Ok(Some(holochain_serialized_bytes::decode(bytes)?)),
            None => Ok(None),
        }
    }

    /// Put V into DB as serialized data
    fn put(&self, writer: &mut Writer, k: &K, v: &V) -> DatabaseResult<()> {
        let buf = holochain_serialized_bytes::encode(v)?;
        let encoded = rusqlite::types::Value::Blob(&buf);
        self.db.put(writer, k.as_ref(), &encoded)?;
        Ok(())
    }

    /// Delete value from DB
    fn delete(&self, writer: &mut Writer, k: &K) -> DatabaseResult<()> {
        Ok(self.db.delete(writer, k.as_ref())?)
    }

    /// Iterate over the underlying persisted data
    fn iter<'env, R: Readable>(&self, reader: &'env R) -> DatabaseResult<SingleIterRaw<'env, V>> {
        todo!("lmdb")
        // Ok(SingleIterRaw::new(
        //     self.db.iter_start(reader)?,
        //     self.db.iter_end(reader)?,
        // ))
    }

    /// Iterate from a key onwards
    fn iter_from<'env, R: Readable>(
        &self,
        reader: &'env R,
        k: K,
    ) -> DatabaseResult<SingleIterRaw<'env, V>> {
        todo!("lmdb")
        // Ok(SingleIterRaw::new(
        //     self.db.iter_from(reader, k)?,
        //     self.db.iter_end(reader)?,
        // ))
    }

    /// Iterate over the underlying persisted data in reverse
    fn iter_reverse<'env, R: Readable>(
        &self,
        reader: &'env R,
    ) -> DatabaseResult<fallible_iterator::Rev<SingleIterRaw<'env, V>>> {
        todo!("lmdb")
        // Ok(SingleIterRaw::new(self.db.iter_start(reader)?, self.db.iter_end(reader)?).rev())
    }
}

impl<V> KvIntStore<V>
where
    V: BufVal,
{
    /// Create a new KvIntBufFresh
    pub fn new(db: IntegerTable) -> Self {
        Self {
            db,
            __phantom: std::marker::PhantomData,
        }
    }

    /// Accessor for raw Rkv DB
    pub fn db(&self) -> IntegerTable {
        self.db.clone()
    }

    // TODO: This should be cfg test but can't because it's in a different crate
    /// Clear db, useful for tests
    pub fn delete_all(&mut self, writer: &mut Writer) -> DatabaseResult<()> {
        Ok(self.db.clear(writer)?)
    }
}
