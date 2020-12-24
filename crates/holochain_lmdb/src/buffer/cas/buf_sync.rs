// FIXME: remove
#![allow(dead_code)]

use crate::buffer::BufferedStore;
use crate::buffer::KvBufUsed;
use crate::env::EnvironmentRead;
use crate::error::DatabaseError;
use crate::error::DatabaseResult;
use crate::fatal_db_hash_integrity_check;
use crate::fresh_reader;
use crate::prelude::*;
use crate::transaction::Readable;
use fallible_iterator::FallibleIterator;
use holo_hash::hash_type::HashTypeSync;
use holo_hash::HasHash;
use holo_hash::HashableContent;
use holo_hash::HoloHashOf;
use holo_hash::HoloHashed;
use holo_hash::PrimitiveHashType;

/// A wrapper around a KvBufFresh where keys are always Addresses,
/// and values are always AddressableContent.
///
/// There is no "CasStore" (which would wrap a `KvStore`), because so far
/// there has been no need for one.
pub struct CasBufUsedSync<C, P = IntegratedPrefix>(KvBufUsed<PrefixHashKey<P>, C>)
where
    C: HashableContent + BufVal + Send + Sync,
    HoloHashOf<C>: BufKey,
    C::HashType: PrimitiveHashType + HashTypeSync + Send + Sync,
    P: PrefixType;

impl<C, P> CasBufUsedSync<C, P>
where
    C: HashableContent + BufVal + Send + Sync,
    HoloHashOf<C>: BufKey,
    C::HashType: PrimitiveHashType + HashTypeSync + Send + Sync,
    P: PrefixType,
{
    /// Create a new CasBufUsedSync
    pub fn new(db: rkv::SingleStore) -> Self {
        Self(KvBufUsed::new(db))
    }

    /// Put a value into the underlying [KvBufUsed]
    pub fn put(&mut self, h: HoloHashed<C>) {
        let key = PrefixHashKey::new(h.as_hash());
        let content = h.into_content();
        // These expects seem valid as it means the hashing is broken
        self.0.put(key, content).expect("Hash should not be empty");
    }

    /// Delete a value from the underlying [KvBufUsed]
    pub fn delete(&mut self, k: HoloHashOf<C>) {
        let k = PrefixHashKey::new(k.as_hash());
        // These expects seem valid as it means the hashing is broken
        self.0.delete(k).expect("Hash key is empty");
    }

    /// Remove a delete from the underlying [KvBufUsed] scratch space
    pub fn cancel_delete(&mut self, k: HoloHashOf<C>) {
        let k = PrefixHashKey::new(k.as_hash());
        // These expects seem valid as it means the hashing is broken
        self.0.cancel_delete(k).expect("Hash key is empty");
    }

    /// Get a value from the underlying [KvBufUsed]
    pub fn get<'r, 'a: 'r, R: Readable>(
        &'a self,
        r: &'r R,
        hash: &'a HoloHashOf<C>,
    ) -> DatabaseResult<Option<HoloHashed<C>>> {
        let k = PrefixHashKey::new(hash.as_hash());
        Ok(if let Some(content) = self.0.get(r, &k)? {
            Some(Self::deserialize_and_hash(hash.as_ref(), content))
        } else {
            None
        })
    }

    /// Check if a value is stored at this key
    pub fn contains<'r, R: Readable>(&self, r: &'r R, k: &HoloHashOf<C>) -> DatabaseResult<bool> {
        let k = PrefixHashKey::new(k.as_hash());
        self.0.contains(r, &k)
    }

    /// Check if a value is in the scratch space
    pub fn contains_in_scratch(&self, k: &HoloHashOf<C>) -> DatabaseResult<bool> {
        let k = PrefixHashKey::new(k.as_hash());
        self.0.contains_in_scratch(&k)
    }

    /// Iterate over the underlying persisted data taking the scratch space into consideration
    pub fn iter_fail<'r, R: Readable>(
        &'r self,
        r: &'r R,
    ) -> DatabaseResult<impl FallibleIterator<Item = HoloHashed<C>, Error = DatabaseError> + 'r>
    {
        Ok(Box::new(self.0.iter(r)?.map(|(h, c)| {
            let k: PrefixHashKey<P> = PrefixHashKey::from_key_bytes_or_friendly_panic(h);
            Ok(Self::deserialize_and_hash(k.as_hash_bytes(), c))
        })))
    }

    fn deserialize_and_hash(hash_bytes: &[u8], content: C) -> HoloHashed<C> {
        let data = HoloHashed::from_content_sync(content);
        fatal_db_hash_integrity_check!(
            "CasBufUsedSync::get",
            hash_bytes,
            data.as_hash().get_raw_39(),
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
pub struct CasBufFreshSync<C, P = IntegratedPrefix>
where
    C: HashableContent + BufVal + Send + Sync,
    HoloHashOf<C>: BufKey,
    C::HashType: PrimitiveHashType + HashTypeSync + Send + Sync,
    P: PrefixType,
{
    env: EnvironmentRead,
    #[shrinkwrap(main_field)]
    inner: CasBufUsedSync<C, P>,
}

impl<C, P> CasBufFreshSync<C, P>
where
    C: HashableContent + BufVal + Send + Sync,
    HoloHashOf<C>: BufKey,
    C::HashType: PrimitiveHashType + HashTypeSync + Send + Sync,
    P: PrefixType,
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

    /// Check if a value is in the scratch space
    pub fn contains_in_scratch(&self, k: &HoloHashOf<C>) -> DatabaseResult<bool> {
        self.inner.contains_in_scratch(k)
    }

    pub fn inner(&self) -> &CasBufUsedSync<C, P> {
        &self.inner
    }
}

impl<C, P> BufferedStore for CasBufUsedSync<C, P>
where
    C: HashableContent + BufVal + Send + Sync,
    C::HashType: PrimitiveHashType + HashTypeSync + Send + Sync,
    P: PrefixType,
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

impl<C, P> BufferedStore for CasBufFreshSync<C, P>
where
    C: HashableContent + BufVal + Send + Sync,
    C::HashType: PrimitiveHashType + HashTypeSync + Send + Sync,
    P: PrefixType,
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
/// Create an CasBufFreshSync with a clone of the scratch
/// from another CasBufFreshSync
impl<P, C> From<&CasBufFreshSync<C, P>> for CasBufFreshSync<C, P>
where
    C: HashableContent + BufVal + Send + Sync,
    HoloHashOf<C>: BufKey,
    C::HashType: PrimitiveHashType + HashTypeSync + Send + Sync,
    P: PrefixType,
{
    fn from(other: &CasBufFreshSync<C, P>) -> Self {
        Self {
            env: other.env.clone(),
            inner: (&other.inner).into(),
        }
    }
}

impl<C, P> From<&CasBufUsedSync<C, P>> for CasBufUsedSync<C, P>
where
    C: HashableContent + BufVal + Send + Sync,
    HoloHashOf<C>: BufKey,
    C::HashType: PrimitiveHashType + HashTypeSync + Send + Sync,
    P: PrefixType,
{
    fn from(other: &CasBufUsedSync<C, P>) -> Self {
        Self((&other.0).into())
    }
}
