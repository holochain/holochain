#![allow(missing_docs)]
use crate::{
    error::{DatabaseError, DatabaseResult},
    transaction::Writer,
};
use serde::{de::DeserializeOwned, Serialize};
use std::hash::Hash;

pub mod cas;
pub mod iter;
pub mod iv;
pub mod kv;
pub mod kvv;

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
pub trait BufKey: Ord + Eq + AsRef<[u8]> + From<Vec<u8>> + Into<Vec<u8>> + Send + Sync {}
impl<T> BufKey for T where T: Ord + Eq + AsRef<[u8]> + From<Vec<u8>> + Into<Vec<u8>> + Send + Sync {}

/// Trait alias for the combination of constraints needed for keys in [IntKvBuf](kv_int::IntKvBuf)
pub trait BufIntKey: Ord + Eq + rkv::store::integer::PrimitiveInt + Send + Sync {}
impl<T> BufIntKey for T where T: Ord + Eq + rkv::store::integer::PrimitiveInt + Send + Sync {}

/// Trait alias for the combination of constraints needed for values in [KvBuf](kv::KvBuf) and [IntKvBuf](kv_int::IntKvBuf)
pub trait BufVal: Clone + Serialize + DeserializeOwned + std::fmt::Debug + Send + Sync {}
impl<T> BufVal for T where T: Clone + Serialize + DeserializeOwned + std::fmt::Debug + Send + Sync {}

/// Trait alias for the combination of constraints needed for values in [KvvBuf]
pub trait BufMultiVal: Hash + Eq + Clone + Serialize + DeserializeOwned + Send + Sync {}
impl<T> BufMultiVal for T where T: Hash + Eq + Clone + Serialize + DeserializeOwned + Send + Sync {}

/// Use this as the key type for LMDB databases which should only have one key.
///
/// This type can only be used as one possible reference
#[derive(derive_more::Display, PartialOrd, Ord, PartialEq, Eq)]
pub struct UnitDbKey;

impl AsRef<[u8]> for UnitDbKey {
    fn as_ref(&self) -> &[u8] {
        ARBITRARY_BYTE_SLICE
    }
}

impl From<Vec<u8>> for UnitDbKey {
    fn from(bytes: Vec<u8>) -> Self {
        assert_eq!(bytes.as_slice(), ARBITRARY_BYTE_SLICE);
        Self
    }
}

impl From<UnitDbKey> for Vec<u8> {
    fn from(_: UnitDbKey) -> Vec<u8> {
        ARBITRARY_BYTE_SLICE.to_vec()
    }
}

impl From<()> for UnitDbKey {
    fn from(_: ()) -> Self {
        Self
    }
}

static ARBITRARY_BYTE_SLICE: &[u8] = &[0];
