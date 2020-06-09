//! Module for items related to aggregating validation_receipts

use holochain_types::composite_hash::EntryHash;
use holochain_state::{
    buffer::KvvBuf,
    error::DatabaseResult,
    exports::MultiStore,
    prelude::Reader,
};
use holochain_serialized_bytes::prelude::*;

/// An individiual validation receipt
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize, SerializedBytes)]
pub struct ValidationReceipt {
}

/// The database/buffer for aggregating validation_receipts sent by remote
/// nodes in charge of storage thereof.
pub struct ValidationReceiptsBuf<'env>(KvvBuf<'env, EntryHash, ValidationReceipt, Reader<'env>>);

impl<'env> ValidationReceiptsBuf<'env> {
    /// ValidationReceiptsBuf constructor given read-only transaction and db ref
    pub fn new(reader: &'env Reader<'env>, db: MultiStore) -> DatabaseResult<Self> {
        Ok(Self(KvvBuf::new(reader, db)?))
    }
}

#[cfg(test)]
mod tests {
}
