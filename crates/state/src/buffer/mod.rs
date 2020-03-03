use crate::error::WorkspaceResult;
use rkv::Writer;

mod cas;
mod kv;
mod kv_int;
mod kvv;

pub use cas::CasBuffer;
pub use kv::KvBuffer;
pub use kv_int::KvIntBuffer;
pub use kvv::KvvBuffer;
use serde::{de::DeserializeOwned, Serialize};
use std::hash::Hash;

/// General trait for transactional stores, exposing only the method which
/// finalizes the transaction. Not currently used, but could be used in Workspaces
/// i.e. iterating over a Vec<dyn StoreBuffer> is all that needs to happen
/// to commit the workspace changes
pub trait StoreBuffer<'env> {
    // fn iter(&self) -> WorkspaceResult<Box<dyn Iterator<Item=(V)> + 'env>>;
    // fn iter_reverse(&self) -> WorkspaceResult<Box<dyn Iterator<Item=(V)> + 'env>>;
    fn finalize(self, writer: &'env mut Writer) -> WorkspaceResult<()>;
}

pub trait BufferKey: Hash + Eq + AsRef<[u8]> {}
impl<T> BufferKey for T where T: Hash + Eq + AsRef<[u8]> {}

pub trait BufferIntKey: Hash + Eq + rkv::store::integer::PrimitiveInt {}
impl<T> BufferIntKey for T where T: Hash + Eq + rkv::store::integer::PrimitiveInt {}

pub trait BufferVal: Clone + Serialize + DeserializeOwned {}
impl<T> BufferVal for T where T: Clone + Serialize + DeserializeOwned {}

pub trait BufferMultiVal: Hash + Eq + Clone + Serialize + DeserializeOwned {}
impl<T> BufferMultiVal for T where T: Hash + Eq + Clone + Serialize + DeserializeOwned {}
