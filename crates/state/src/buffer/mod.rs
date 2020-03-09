mod cas;
mod kv;
mod kv_int;
mod kvv;

use crate::prelude::Writer;
pub use cas::CasBuf;
pub use kv::KvBuf;
pub use kv_int::IntKvBuf;
pub use kvv::KvvBuf;
use serde::{de::DeserializeOwned, Serialize};
use std::hash::Hash;

/// General trait for transactional stores, exposing only the method which
/// adds changes to the write transaction. This generalization is not really used,
/// but could be used in Workspaces i.e. iterating over a Vec<dyn BufferedStore>
/// is all that needs to happen to commit the workspace changes
pub trait BufferedStore<'env> {
    type Error: std::error::Error;
    // fn iter(&self) -> WorkspaceResult<Box<dyn Iterator<Item=(V)> + 'env>>;
    // fn iter_reverse(&self) -> WorkspaceResult<Box<dyn Iterator<Item=(V)> + 'env>>;
    fn flush_to_txn(self, writer: &'env mut Writer) -> Result<(), Self::Error>;
}

pub trait BufKey: Hash + Eq + AsRef<[u8]> {}
impl<T> BufKey for T where T: Hash + Eq + AsRef<[u8]> {}

pub trait BufIntKey: Hash + Eq + rkv::store::integer::PrimitiveInt {}
impl<T> BufIntKey for T where T: Hash + Eq + rkv::store::integer::PrimitiveInt {}

pub trait BufVal: Clone + Serialize + DeserializeOwned {}
impl<T> BufVal for T where T: Clone + Serialize + DeserializeOwned {}

pub trait BufMultiVal: Hash + Eq + Clone + Serialize + DeserializeOwned {}
impl<T> BufMultiVal for T where T: Hash + Eq + Clone + Serialize + DeserializeOwned {}
