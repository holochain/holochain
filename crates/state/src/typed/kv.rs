use crate::{
    buffer::{BufKey, BufVal},
    error::{DatabaseError, DatabaseResult},
    prelude::{Readable, Writer},
};
use rkv::SingleStore;

/// Wrapper around an rkv SingleStore which provides strongly typed values
// #[derive(Shrinkwrap)]
pub struct Kv<K, V>
where
    K: BufKey,
    V: BufVal,
{
    // #[shrinkwrap(main_field)]
    db: SingleStore,
    _phantom: std::marker::PhantomData<(K, V)>,
}

impl<K, V> Kv<K, V>
where
    K: BufKey,
    V: BufVal,
{
    /// Create a new IntKvBuf from a read-only transaction and a database reference
    pub fn new(db: SingleStore) -> DatabaseResult<Self> {
        Ok(Self {
            db,
            _phantom: std::marker::PhantomData,
        })
    }

    /// Fetch data from DB, deserialize into V type
    pub fn get<R: Readable>(&self, reader: &R, k: &K) -> DatabaseResult<Option<V>> {
        match self.db.get(reader, k)? {
            Some(rkv::Value::Blob(buf)) => Ok(Some(rmp_serde::from_read_ref(buf)?)),
            None => Ok(None),
            Some(_) => Err(DatabaseError::InvalidValue),
        }
    }

    /// Put V into DB as serialized data
    pub fn put(&self, writer: &mut Writer, k: &K, v: &V) -> DatabaseResult<()> {
        let buf = rmp_serde::to_vec_named(v)?;
        let encoded = rkv::Value::Blob(&buf);
        self.db.put(writer, k, &encoded)?;
        Ok(())
    }

    /// Delete value from DB
    pub fn delete(&self, writer: &mut Writer, k: &K) -> DatabaseResult<()> {
        Ok(self.db.delete(writer, k)?)
    }
}
