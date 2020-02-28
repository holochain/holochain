use super::{kv::KvBuffer, StoreBuffer, BufferVal};
use crate::error::WorkspaceResult;
use rkv::{Rkv, Writer};
use serde::{de::DeserializeOwned, Serialize};
use sx_types::prelude::{Address, AddressableContent};

/// A wrapper around a KvBuffer where keys are always Addresses,
/// and values are always AddressableContent.
pub struct CasBuffer<'env, V>(KvBuffer<'env, Address, V>)
where
    V: BufferVal + AddressableContent;

impl<'env, V> CasBuffer<'env, V>
where
    V: BufferVal + AddressableContent,
{
    /// Create or open DB if it exists.
    /// CAREFUL with this! Calling create() during a transaction seems to cause a deadlock
    pub fn create(env: &'env Rkv, name: &str) -> WorkspaceResult<Self> {
        Ok(Self(KvBuffer::create(env, name)?))
    }

    /// Open an existing DB. Will cause an error if the DB was not created already.
    pub fn open(env: &'env Rkv, name: &str) -> WorkspaceResult<Self> {
        Ok(Self(KvBuffer::open(env, name)?))
    }

    pub fn get(&self, k: &Address) -> WorkspaceResult<Option<V>> {
        self.0.get(k)
    }

    pub fn put(&mut self, v: V) -> () {
        self.0.put(v.address(), v)
    }

    pub fn delete(&mut self, k: Address) -> () {
        self.0.delete(k)
    }
}

impl<'env, V> StoreBuffer<'env> for CasBuffer<'env, V>
where
    V: BufferVal + AddressableContent,
{
    fn finalize(self, writer: &'env mut Writer) -> WorkspaceResult<()> {
        self.0.finalize(writer)?;
        Ok(())
    }
}
