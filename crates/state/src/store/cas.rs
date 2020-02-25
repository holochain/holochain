use sx_types::prelude::{AddressableContent, Address};
use super::kv::KvStore;
use serde::{de::DeserializeOwned, Serialize};
use rkv::Rkv;
use crate::error::WorkspaceResult;

/// A wrapper around a KvStore where keys are always Addresses,
/// and values are always AddressableContent.
pub struct Cas<'env, V>(KvStore<'env, Address, V>)
where V: AddressableContent + Clone + Serialize + DeserializeOwned;

impl<'env, V> Cas<'env, V>
where V: AddressableContent + Clone + Serialize + DeserializeOwned
{
    /// Create or open DB if it exists.
    /// CAREFUL with this! Calling create() during a transaction seems to cause a deadlock
    pub fn create(env: &'env Rkv, name: &str) -> WorkspaceResult<Self> {
        Ok(Self(KvStore::create(env, name)?))
    }

    /// Open an existing DB. Will cause an error if the DB was not created already.
    pub fn open(env: &'env Rkv, name: &str) -> WorkspaceResult<Self> {
        Ok(Self(KvStore::open(env, name)?))
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
