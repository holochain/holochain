use super::{
    kv::{KvBuf, SingleIter},
    BufKey, BufVal, BufferedStore,
};
use crate::{
    error::{DatabaseError, DatabaseResult},
    fatal_db_deserialize_check, fatal_db_hash_check,
    prelude::{Reader, Writer},
    transaction::Readable,
};
use futures::future::{BoxFuture, FutureExt, OptionFuture};
use holo_hash::Hashable;
use must_future::MustBoxFuture;

/// A wrapper around a KvBuf where keys are always Addresses,
/// and values are always AddressableContent.
pub struct CasBuf<'env, H>(KvBuf<'env, H::HashType, H::Content, Reader<'env>>)
where
    H: Hashable + Send,
    H::HashType: BufKey,
    H::Content: BufVal + Send + Sync;

impl<'env, H> CasBuf<'env, H>
where
    H: Hashable + Send,
    H::HashType: BufKey,
    H::Content: BufVal + Send + Sync,
{
    /// Create a new CasBuf from a read-only transaction and a database reference
    pub fn new(reader: &'env Reader<'env>, db: rkv::SingleStore) -> DatabaseResult<Self> {
        Ok(Self(KvBuf::new(reader, db)?))
    }

    /// Get a value from the underlying [KvBuf]
    pub fn get<'a>(&'env self, k: &'env H::HashType) -> BoxFuture<'env, DatabaseResult<Option<H>>> {
        async move {
            Ok(if let Some(content) = self.0.get(k)? {
                let data =
                    fatal_db_deserialize_check!("CasBuf::get", k, H::with_data(content).await);
                fatal_db_hash_check!("CasBuf::get", k, data.as_hash());
                Some(data)
            } else {
                None
            })
        }
        .boxed()
    }

    /// Put a value into the underlying [KvBuf]
    pub fn put(&mut self, h: H) {
        let (content, hash) = h.into_inner();
        self.0.put(hash, content)
    }

    /// Delete a value from the underlying [KvBuf]
    pub fn delete(&mut self, k: H::HashType) {
        self.0.delete(k)
    }

    /// Iterate over the underlying persisted data, NOT taking the scratch space into consideration
    pub fn iter_raw(&self) -> DatabaseResult<Box<dyn Iterator<Item = H>>> {
        todo!("hook up")
        // self.0.iter_raw()
    }
}

impl<'env, H> BufferedStore<'env> for CasBuf<'env, H>
where
    H: Hashable + Send,
    H::HashType: BufKey,
    H::Content: BufVal + Send + Sync,
{
    type Error = DatabaseError;

    fn flush_to_txn(self, writer: &'env mut Writer) -> DatabaseResult<()> {
        self.0.flush_to_txn(writer)?;
        Ok(())
    }
}
