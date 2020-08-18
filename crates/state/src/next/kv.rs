use crate::{
    error::{DatabaseError, DatabaseResult},
    transaction::Writer,
};
use serde::{de::DeserializeOwned, Serialize};
use std::hash::Hash;

mod buf;
mod iter;
mod store;
pub use buf::*;
pub use iter::*;
pub use store::*;

#[cfg(test)]
mod test;

// Empty keys break lmdb
pub(super) fn check_empty_key<K: BufKey>(k: &K) -> DatabaseResult<()> {
    if k.as_ref().is_empty() {
        Err(DatabaseError::EmptyKey)
    } else {
        Ok(())
    }
}

/// General trait for transactional stores, exposing only the method which
/// adds changes to the write transaction. This generalization is not really used,
/// but could be used in Workspaces i.e. iterating over a Vec<dyn BufferedStore>
/// is all that needs to happen to commit the workspace changes
pub trait BufferedStore {
    /// The error type for `flush_to_txn` errors
    type Error: std::error::Error;

    /// Flush the scratch space to the read-write transaction, staging the changes
    /// for an actual database update
    fn flush_to_txn(self, writer: &mut Writer) -> Result<(), Self::Error>;

    /// Specifies whether there are actually changes to flush. If not, the
    /// flush_to_txn method may decide to do nothing.
    fn is_clean(&self) -> bool {
        false
    }
}

/// Trait alias for the combination of constraints needed for keys in [KvBuf] and [KvvBuf]
pub trait BufKey: Hash + Ord + Eq + AsRef<[u8]> {}
impl<T> BufKey for T where T: Hash + Ord + Eq + AsRef<[u8]> {}

/// Trait alias for the combination of constraints needed for keys in [IntKvBuf](kv_int::IntKvBuf)
pub trait BufIntKey: Hash + Ord + Eq + rkv::store::integer::PrimitiveInt {}
impl<T> BufIntKey for T where T: Hash + Ord + Eq + rkv::store::integer::PrimitiveInt {}

/// Trait alias for the combination of constraints needed for values in [KvBuf](kv::KvBuf) and [IntKvBuf](kv_int::IntKvBuf)
pub trait BufVal: Clone + Serialize + DeserializeOwned + std::fmt::Debug {}
impl<T> BufVal for T where T: Clone + Serialize + DeserializeOwned + std::fmt::Debug {}

/// Trait alias for the combination of constraints needed for values in [KvvBuf]
pub trait BufMultiVal: Hash + Eq + Clone + Serialize + DeserializeOwned {}
impl<T> BufMultiVal for T where T: Hash + Eq + Clone + Serialize + DeserializeOwned {}
