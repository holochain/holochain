use crate::buffer::iter::SingleIterRaw;
use crate::error::DatabaseResult;
use crate::prelude::*;

pub trait KvStoreT<K, V> {
    /// Fetch data from DB as raw byte slice
    fn get_bytes<'env, R: Readable>(
        &self,
        reader: &'env R,
        k: &K,
    ) -> DatabaseResult<Option<&'env [u8]>>;

    /// Fetch data from DB, deserialize into V type
    fn get<R: Readable>(&self, reader: &R, k: &K) -> DatabaseResult<Option<V>>;

    /// Put V into DB as serialized data
    fn put(&self, writer: &mut Writer, k: &K, v: &V) -> DatabaseResult<()>;

    /// Delete value from DB by key
    fn delete(&self, writer: &mut Writer, k: &K) -> DatabaseResult<()>;

    /// Iterate over the underlying persisted data
    fn iter<'env, R: Readable>(&self, reader: &'env R) -> DatabaseResult<SingleIterRaw<'env, V>>;

    /// Iterate from a key onwards
    fn iter_from<'env, R: Readable>(
        &self,
        reader: &'env R,
        k: K,
    ) -> DatabaseResult<SingleIterRaw<'env, V>>;

    /// Iterate over the underlying persisted data in reverse
    #[deprecated = "just use rev()"]
    fn iter_reverse<'env, R: Readable>(
        &self,
        reader: &'env R,
    ) -> DatabaseResult<fallible_iterator::Rev<SingleIterRaw<'env, V>>>;
}
