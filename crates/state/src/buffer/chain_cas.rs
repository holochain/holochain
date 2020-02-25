use super::cas::CasBuffer;
use crate::error::WorkspaceResult;
use rkv::Rkv;
use serde::{de::DeserializeOwned, Serialize};
use sx_types::{
    chain_header::ChainHeader,
    entry::Entry,
    prelude::{Address, AddressableContent},
};
/// A convenient pairing of two CasBuffers, one for entries and one for headers
pub struct ChainCasBuffer<'env> {
    entries: CasBuffer<'env, Entry>,
    headers: CasBuffer<'env, ChainHeader>,
}

impl<'env> ChainCasBuffer<'env> {
    /// Create or open DB if it exists.
    /// CAREFUL with this! Calling create() during a transaction seems to cause a deadlock
    pub fn create(env: &'env Rkv, prefix: &str) -> WorkspaceResult<Self> {
        Ok(Self {
            entries: CasBuffer::create(env, &format!("{}-entries", prefix))?,
            headers: CasBuffer::create(env, &format!("{}-headers", prefix))?,
        })
    }

    /// Open an existing DB. Will cause an error if the DB was not created already.
    pub fn open(env: &'env Rkv, prefix: &str) -> WorkspaceResult<Self> {
        Ok(Self {
            entries: CasBuffer::open(env, &format!("{}-entries", prefix))?,
            headers: CasBuffer::open(env, &format!("{}-headers", prefix))?,
        })
    }

    pub fn get_entry(&self, k: &Address) -> WorkspaceResult<Option<Entry>> {
        self.entries.get(k)
    }

    pub fn get_header(&self, k: &Address) -> WorkspaceResult<Option<ChainHeader>> {
        self.headers.get(k)
    }

    pub fn put_entry(&mut self, v: Entry) -> () {
        self.entries.put(v)
    }

    pub fn put_header(&mut self, v: ChainHeader) -> () {
        self.headers.put(v)
    }

    pub fn delete_entry(&mut self, k: Address) -> () {
        self.entries.delete(k)
    }

    pub fn delete_header(&mut self, k: Address) -> () {
        self.headers.delete(k)
    }
}
