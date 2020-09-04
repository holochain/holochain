// FIXME: remove
#![allow(dead_code)]

use crate::{
    buffer::{BufferedStore, KvBufUsed},
    env::EnvironmentRead,
    error::{DatabaseError, DatabaseResult},
    fatal_db_hash_integrity_check, fresh_reader,
    prelude::*,
    transaction::Readable,
};
use fallible_iterator::FallibleIterator;
use holo_hash::{
    hash_type::HashTypeSync, HasHash, HashableContent, HoloHashOf, HoloHashed, PrimitiveHashType,
};

/// A wrapper around a KvBufFresh where keys are always Addresses,
/// and values are always AddressableContent.
///
/// There is no "CasStore" (which would wrap a `KvStore`), because so far
/// there has been no need for one.
pub struct CasBufUsedSync<C>(KvBufUsed<HoloHashOf<C>, C>)
where
    C: HashableContent + BufVal + Send + Sync,
    HoloHashOf<C>: BufKey,
    C::HashType: PrimitiveHashType + HashTypeSync + Send + Sync;

impl<C> CasBufUsedSync<C>
where
    C: HashableContent + BufVal + Send + Sync,
    HoloHashOf<C>: BufKey,
    C::HashType: PrimitiveHashType + HashTypeSync + Send + Sync,
{
    /// Create a new CasBufUsedSync
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
    pub fn get<'r, 'a: 'r, R: Readable + Send + Sync>(
        &'a self,
        r: &'r R,
        hash: &'a HoloHashOf<C>,
    ) -> DatabaseResult<Option<HoloHashed<C>>> {
        Ok(if let Some(content) = self.0.get(r, hash)? {
            Some(Self::deserialize_and_hash(hash.get_full_bytes(), content))
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
        Ok(Box::new(
            self.0
                .iter(r)?
                .map(|(h, c)| Ok(Self::deserialize_and_hash(&h[..], c))),
        ))
    }

    fn deserialize_and_hash(hash_bytes: &[u8], content: C) -> HoloHashed<C> {
        let data = HoloHashed::from_content_sync(content);
        fatal_db_hash_integrity_check!(
            "CasBufUsedSync::get",
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

#[derive(shrinkwraprs::Shrinkwrap)]
#[shrinkwrap(mutable, unsafe_ignore_visibility)]
pub struct CasBufFreshSync<C>
where
    C: HashableContent + BufVal + Send + Sync,
    HoloHashOf<C>: BufKey,
    C::HashType: PrimitiveHashType + HashTypeSync + Send + Sync,
{
    env: EnvironmentRead,
    #[shrinkwrap(main_field)]
    inner: CasBufUsedSync<C>,
}

impl<C> CasBufFreshSync<C>
where
    C: HashableContent + BufVal + Send + Sync,
    HoloHashOf<C>: BufKey,
    C::HashType: PrimitiveHashType + HashTypeSync + Send + Sync,
{
    /// Create a new CasBufFreshSync
    pub fn new(env: EnvironmentRead, db: rkv::SingleStore) -> Self {
        Self {
            env,
            inner: CasBufUsedSync::new(db),
        }
    }

    pub fn env(&self) -> &EnvironmentRead {
        &self.env
    }

    /// Get a value from the underlying [KvBufFresh]
    pub fn get<'a>(&'a self, hash: &'a HoloHashOf<C>) -> DatabaseResult<Option<HoloHashed<C>>> {
        fresh_reader!(self.env, |r| self.inner.get(&r, hash))
    }

    /// Check if a value is stored at this key
    pub fn contains(&self, k: &HoloHashOf<C>) -> DatabaseResult<bool> {
        fresh_reader!(self.env, |r| self.inner.contains(&r, k))
    }
}

impl<C> BufferedStore for CasBufUsedSync<C>
where
    C: HashableContent + BufVal + Send + Sync,
    C::HashType: PrimitiveHashType + HashTypeSync + Send + Sync,
{
    type Error = DatabaseError;

    fn is_clean(&self) -> bool {
        self.0.is_clean()
    }

    fn flush_to_txn_ref(&mut self, writer: &mut Writer) -> DatabaseResult<()> {
        self.0.flush_to_txn_ref(writer)?;
        Ok(())
    }
}

impl<C> BufferedStore for CasBufFreshSync<C>
where
    C: HashableContent + BufVal + Send + Sync,
    C::HashType: PrimitiveHashType + HashTypeSync + Send + Sync,
{
    type Error = DatabaseError;

    fn is_clean(&self) -> bool {
        self.inner.is_clean()
    }

    fn flush_to_txn_ref(&mut self, writer: &mut Writer) -> DatabaseResult<()> {
        self.inner.flush_to_txn_ref(writer)?;
        Ok(())
    }
}
