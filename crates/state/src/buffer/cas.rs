use super::{
    kv::{KvBuf, SingleIter},
    BufVal, BufferedStore,
};
use crate::{
    error::{WorkspaceError, WorkspaceResult},
    reader::Readable,
};
use rkv::Writer;

use sx_types::prelude::{Address, AddressableContent};

/// A wrapper around a KvBuf where keys are always Addresses,
/// and values are always AddressableContent.
pub struct CasBuf<'env, V, R>(KvBuf<'env, Address, V, R>)
where
    V: BufVal + AddressableContent,
    R: Readable;

impl<'env, V, R> CasBuf<'env, V, R>
where
    V: BufVal + AddressableContent,
    R: Readable,
{
    pub fn new(reader: &'env R, db: rkv::SingleStore) -> WorkspaceResult<Self> {
        Ok(Self(KvBuf::new(reader, db)?))
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

    /// Iterate over the underlying persisted data, NOT taking the scratch space into consideration
    pub fn iter_raw(&self) -> WorkspaceResult<SingleIter<V>> {
        self.0.iter_raw()
    }
}

impl<'env, V, R> BufferedStore<'env> for CasBuf<'env, V, R>
where
    V: BufVal + AddressableContent,
    R: Readable,
{
    type Error = WorkspaceError;

    fn flush_to_txn(self, writer: &'env mut Writer) -> WorkspaceResult<()> {
        self.0.flush_to_txn(writer)?;
        Ok(())
    }
}
