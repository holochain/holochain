use super::{kv::KvBuffer, BufferVal, StoreBuffer};
use crate::{error::WorkspaceResult, Readable};
use rkv::Writer;

use sx_types::prelude::{Address, AddressableContent};

/// A wrapper around a KvBuffer where keys are always Addresses,
/// and values are always AddressableContent.
pub struct CasBuffer<'env, V, R>(KvBuffer<'env, Address, V, R>)
where
    V: BufferVal + AddressableContent,
    R: Readable;

impl<'env, V, R> CasBuffer<'env, V, R>
where
    V: BufferVal + AddressableContent,
    R: Readable,
{
    pub fn new(reader: &'env R, db: rkv::SingleStore) -> WorkspaceResult<Self> {
        Ok(Self(KvBuffer::new(reader, db)?))
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

impl<'env, V, R> StoreBuffer<'env> for CasBuffer<'env, V, R>
where
    V: BufferVal + AddressableContent,
    R: Readable,
{
    fn finalize(self, writer: &'env mut Writer) -> WorkspaceResult<()> {
        self.0.finalize(writer)?;
        Ok(())
    }
}
