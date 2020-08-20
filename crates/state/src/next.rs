#![allow(missing_docs)]
use crate::{
    error::{DatabaseError, DatabaseResult},
    transaction::Writer,
};
use serde::{de::DeserializeOwned, Serialize};
use std::hash::Hash;

pub mod cas;
pub mod iter;
pub mod kv;
pub mod kvv;

// Empty keys break lmdb
pub(super) fn check_empty_key<K: AsRef<[u8]>>(k: &K) -> DatabaseResult<()> {
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

#[derive(Copy, PartialOrd, Ord, PartialEq, Eq, Clone, Serialize, serde::Deserialize)]
pub struct IntKey([u8; 4]);

impl rkv::store::integer::PrimitiveInt for IntKey {}

impl From<Vec<u8>> for IntKey {
    fn from(vec: Vec<u8>) -> IntKey {
        use std::convert::TryInto;
        let boxed_slice = vec.into_boxed_slice();
        let boxed_array: Box<[u8; 4]> = match boxed_slice.try_into() {
            Ok(ba) => ba,
            Err(o) => panic!("Expected a Vec of length {} but it was {}", 4, o.len()),
        };
        IntKey(*boxed_array)
    }
}

impl From<IntKey> for Vec<u8> {
    fn from(key: IntKey) -> Vec<u8> {
        key.as_ref().to_owned()
    }
}

impl AsRef<[u8]> for IntKey {
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}

/// Trait alias for the combination of constraints needed for keys in [KvStore] and [KvvStore]
pub trait BufKey: Ord + Eq + AsRef<[u8]> + From<Vec<u8>> + Into<Vec<u8>> + Send + Sync {}
impl<T> BufKey for T where T: Ord + Eq + AsRef<[u8]> + From<Vec<u8>> + Into<Vec<u8>> + Send + Sync {}

/// Trait alias for the combination of constraints needed for keys in [KvIntStore](kv_int::KvIntStore)
pub trait BufIntKey: Ord + Eq + rkv::store::integer::PrimitiveInt + Send + Sync {}
impl<T> BufIntKey for T where T: Ord + Eq + rkv::store::integer::PrimitiveInt + Send + Sync {}

/// Trait alias for the combination of constraints needed for values in [KvStore](kv::KvStore) and [KvIntStore](kv_int::KvIntStore)
pub trait BufVal: Clone + Serialize + DeserializeOwned + std::fmt::Debug + Send + Sync {}
impl<T> BufVal for T where T: Clone + Serialize + DeserializeOwned + std::fmt::Debug + Send + Sync {}

/// Trait alias for the combination of constraints needed for values in [KvvStore]
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
