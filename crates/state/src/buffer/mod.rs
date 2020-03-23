//! This crate provides the elementary BufferedStores:
//!
//! - [KvBuffer]: a SingleStore with a scratch space
//! - [KvIntBuffer]: an IntegerStore with a scratch space
//! - [KvvBuffer]: a MultiStore with a scratch space
//! - [CasBuffer]: a [KvBuffer] which enforces that keys must be the "address" of the values (content)

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
    /// The error type for `flush_to_txn` errors
    type Error: std::error::Error;

    /// Flush the scratch space to the read-write transaction, staging the changes
    /// for an actual database update
    fn flush_to_txn(self, writer: &'env mut Writer) -> Result<(), Self::Error>;
}

/// Trait alias for the combination of constraints needed for keys in [KvBuf] and [KvvBuf]
pub trait BufKey: Hash + Eq + AsRef<[u8]> {}
impl<T> BufKey for T where T: Hash + Eq + AsRef<[u8]> {}

/// Trait alias for the combination of constraints needed for keys in [IntKvBuf]
pub trait BufIntKey: Hash + Eq + rkv::store::integer::PrimitiveInt {}
impl<T> BufIntKey for T where T: Hash + Eq + rkv::store::integer::PrimitiveInt {}

/// Trait alias for the combination of constraints needed for values in [KvBuf] and [IntKvBuf]
pub trait BufVal: Clone + Serialize + DeserializeOwned {}
impl<T> BufVal for T where T: Clone + Serialize + DeserializeOwned {}

/// Trait alias for the combination of constraints needed for values in [KvvBuf]
pub trait BufMultiVal: Hash + Eq + Clone + Serialize + DeserializeOwned {}
impl<T> BufMultiVal for T where T: Hash + Eq + Clone + Serialize + DeserializeOwned {}
