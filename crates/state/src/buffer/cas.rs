use super::{kv::KvBuf, BufKey, BufVal, BufferedStore};
use crate::{
    error::{DatabaseError, DatabaseResult},
    fatal_db_hash_integrity_check,
    prelude::{Reader, Writer},
};
use fallible_iterator::FallibleIterator;
use futures::future::FutureExt;
use holo_hash::{HasHash, HashableContent, HoloHashOf, HoloHashed};
use must_future::MustBoxFuture;

/// A wrapper around a KvBuf where keys are always Addresses,
/// and values are always AddressableContent.
pub struct CasBuf<'env, C>(KvBuf<'env, HoloHashOf<C>, C, Reader<'env>>)
where
    C: HashableContent + BufVal + Send + Sync,
    HoloHashOf<C>: BufKey,
    C::HashType: Send + Sync;

impl<'env, C> CasBuf<'env, C>
where
    C: HashableContent + BufVal + Send + Sync,
    HoloHashOf<C>: BufKey,
    C::HashType: Send + Sync,
{
    /// Create a new CasBuf from a read-only transaction and a database reference
    pub fn new(reader: &'env Reader<'env>, db: rkv::SingleStore) -> DatabaseResult<Self> {
        Ok(Self(KvBuf::new(reader, db)?))
    }

    /// Get a value from the underlying [KvBuf]
    pub fn get(
        &'env self,
        hash: &'env HoloHashOf<C>,
    ) -> MustBoxFuture<'env, DatabaseResult<Option<HoloHashed<C>>>> {
        async move {
            Ok(if let Some(content) = self.0.get(hash)? {
                Some(Self::deserialize_and_hash(hash.get_full_bytes(), content).await)
            } else {
                None
            })
        }
        .boxed()
        .into()
    }

    /// Put a value into the underlying [KvBuf]
    pub fn put(&mut self, h: HoloHashed<C>) {
        let (content, hash) = h.into_inner();
        // These expects seem valid as it means the hashing is broken
        self.0.put(hash, content).expect("Hash should not be empty");
    }

    /// Delete a value from the underlying [KvBuf]
    pub fn delete(&mut self, k: HoloHashOf<C>) {
        // These expects seem valid as it means the hashing is broken
        self.0.delete(k).expect("Hash key is empty");
    }

    /// Check if a value is stored at this key
    pub fn contains(&self, k: &HoloHashOf<C>) -> DatabaseResult<bool> {
        self.0.contains(k)
    }

    /// Iterate over the underlying persisted data taking the scratch space into consideration
    pub fn iter_fail(
        &'env self,
    ) -> DatabaseResult<Box<dyn FallibleIterator<Item = HoloHashed<C>, Error = DatabaseError> + 'env>>
    {
        Ok(Box::new(self.0.iter()?.map(|(h, c)| {
            Ok(Self::deserialize_and_hash_blocking(&h[..], c))
        })))
    }

    #[cfg(test)]
    /// Iterate over the underlying persisted data, NOT taking the scratch space into consideration
    pub fn iter_fail_raw(
        &'env self,
    ) -> DatabaseResult<Box<dyn FallibleIterator<Item = HoloHashed<C>, Error = DatabaseError> + 'env>>
    {
        Ok(Box::new(self.0.iter_raw()?.map(|(h, c)| {
            Ok(Self::deserialize_and_hash_blocking(h, c))
        })))
    }

    fn deserialize_and_hash_blocking(hash: &[u8], content: C) -> HoloHashed<C> {
        tokio_safe_block_on::tokio_safe_block_on(
            Self::deserialize_and_hash(hash, content),
            std::time::Duration::from_millis(500),
        )
        .expect("TODO: make into stream")
        // TODO: make this a stream?
    }

    async fn deserialize_and_hash(hash_bytes: &[u8], content: C) -> HoloHashed<C> {
        let data = HoloHashed::from_content(content).await;
        fatal_db_hash_integrity_check!(
            "CasBuf::get",
            hash_bytes,
            data.as_hash().get_full_bytes(),
            data.as_content(),
        );
        data
    }

    // TODO: This should be cfg test but can't because it's in a different crate
    /// Clear all scratch and db, useful for tests
    pub fn clear_all(&mut self, writer: &mut Writer) -> DatabaseResult<()> {
        self.0.clear_all(writer)
    }
}

impl<'env, C> BufferedStore<'env> for CasBuf<'env, C>
where
    C: HashableContent + BufVal + Send + Sync,
    C::HashType: Send + Sync,
{
    type Error = DatabaseError;

    fn is_clean(&self) -> bool {
        self.0.is_clean()
    }

    fn flush_to_txn(self, writer: &'env mut Writer) -> DatabaseResult<()> {
        self.0.flush_to_txn(writer)?;
        Ok(())
    }
}
