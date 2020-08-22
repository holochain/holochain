use crate::{
    buffer::{kv::KvBufFresh, BufferedStore, KvBufUsed},
    env::EnvironmentRead,
    error::{DatabaseError, DatabaseResult},
    fatal_db_hash_integrity_check, fresh_reader,
    prelude::*,
    transaction::Readable,
};
use fallible_iterator::FallibleIterator;
use futures::future::FutureExt;
use holo_hash::{HasHash, HashableContent, HoloHashOf, HoloHashed, PrimitiveHashType};
use must_future::MustBoxFuture;

/// A wrapper around a KvBufFresh where keys are always Addresses,
/// and values are always AddressableContent.
///
/// There is no "CasStore" (which would wrap a `KvStore`), because so far
/// there has been no need for one.
pub struct CasBufUsed<C>(KvBufUsed<HoloHashOf<C>, C>)
where
    C: HashableContent + BufVal + Send + Sync,
    HoloHashOf<C>: BufKey,
    C::HashType: PrimitiveHashType + Send + Sync;

impl<C> CasBufUsed<C>
where
    C: HashableContent + BufVal + Send + Sync,
    HoloHashOf<C>: BufKey,
    C::HashType: PrimitiveHashType + Send + Sync,
{
    /// Create a new CasBufUsed
    pub fn new(db: rkv::SingleStore) -> Self {
        Self(KvBufUsed::new(db))
    }

    /// Put a value into the underlying [KvBufUsed]
    pub fn put(&mut self, h: HoloHashed<C>) {
        let (content, hash) = h.into_inner();
        // These expects seem valid as it means the hashing is broken
        self.0.put(hash, content).expect("Hash should not be empty");
    }

    /// Delete a value from the underlying [KvBufUsed]
    pub fn delete(&mut self, k: HoloHashOf<C>) {
        // These expects seem valid as it means the hashing is broken
        self.0.delete(k).expect("Hash key is empty");
    }

    /// Get a value from the underlying [KvBufUsed]
    pub async fn get<'r, 'a: 'r, R: Readable + Send + Sync>(
        &'a self,
        r: &'r R,
        hash: &'a HoloHashOf<C>,
    ) -> DatabaseResult<Option<HoloHashed<C>>> {
        Ok(if let Some(content) = self.0.get(r, hash)? {
            Some(Self::deserialize_and_hash(hash.get_full_bytes(), content).await)
        } else {
            None
        })
    }

    /// Check if a value is stored at this key
    pub fn contains<'r, R: Readable>(&self, r: &'r R, k: &HoloHashOf<C>) -> DatabaseResult<bool> {
        self.0.contains(r, k)
    }

    /// Iterate over the underlying persisted data taking the scratch space into consideration
    pub fn iter_fail<'r, R: Readable>(
        &'r self,
        r: &'r R,
    ) -> DatabaseResult<impl FallibleIterator<Item = HoloHashed<C>, Error = DatabaseError> + 'r>
    {
        Ok(Box::new(self.0.iter(r)?.map(|(h, c)| {
            Ok(Self::deserialize_and_hash_blocking(&h[..], c))
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
            "CasBufUsed::get",
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

pub struct CasBufFresh<C>
where
    C: HashableContent + BufVal + Send + Sync,
    HoloHashOf<C>: BufKey,
    C::HashType: PrimitiveHashType + Send + Sync,
{
    env: EnvironmentRead,
    inner: CasBufUsed<C>,
}

impl<C> CasBufFresh<C>
where
    C: HashableContent + BufVal + Send + Sync,
    HoloHashOf<C>: BufKey,
    C::HashType: PrimitiveHashType + Send + Sync,
{
    /// Create a new CasBufFresh
    pub fn new(env: EnvironmentRead, db: rkv::SingleStore) -> Self {
        Self {
            env,
            inner: CasBufUsed::new(db),
        }
    }

    pub fn env(&self) -> &EnvironmentRead {
        &self.0.env()
    }

    /// Get a value from the underlying [KvBufFresh]
    pub async fn get<'a>(
        &'a self,
        hash: &'a HoloHashOf<C>,
    ) -> DatabaseResult<Option<HoloHashed<C>>> {
        fresh_reader!(self.env, |r| async move { self.inner.get(&r, hash).await }).await
    }

    /// Check if a value is stored at this key
    pub async fn contains(&self, k: &HoloHashOf<C>) -> DatabaseResult<bool> {
        self.0.contains(k).await
    }
}

impl<C> BufferedStore for CasBufUsed<C>
where
    C: HashableContent + BufVal + Send + Sync,
    C::HashType: PrimitiveHashType + Send + Sync,
{
    type Error = DatabaseError;

    fn is_clean(&self) -> bool {
        self.0.is_clean()
    }

    fn flush_to_txn(self, writer: &mut Writer) -> DatabaseResult<()> {
        self.0.flush_to_txn(writer)?;
        Ok(())
    }
}
