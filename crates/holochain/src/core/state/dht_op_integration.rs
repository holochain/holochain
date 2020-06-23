//! Database buffers and type related to [DhtOp]s

use crate::core::workflow::produce_dht_ops_workflow::dht_op::DhtOpLight;
use holo_hash::DhtOpHash;
use holochain_serialized_bytes::prelude::*;
use holochain_state::{buffer::KvBuf, prelude::Reader};
use holochain_types::{
    composite_hash::AnyDhtHash, dht_op::DhtOp, validate::ValidationStatus, Timestamp,
};

/// Buffer for accessing [DhtOp]s that you authored and finding the amount of validation receipts
pub type AuthoredDhtOps<'env> = KvBuf<'env, DhtOpHash, u32, Reader<'env>>;

/// Queue of ops ready to be integrated
pub type IntegrationQueue<'env> =
    KvBuf<'env, IntegrationQueueKey, IntegrationQueueValue, Reader<'env>>;

/// [DhtOp]s that have already been integrated
pub type IntegratedDhtOps<'env> = KvBuf<'env, DhtOpHash, IntegrationValue, Reader<'env>>;

/// Key for the Integration Queue that ensures they are ordered by time
#[derive(Hash, Eq, PartialEq)]
pub struct IntegrationQueueKey(SerializedBytes);

#[derive(serde::Deserialize, serde::Serialize, SerializedBytes)]
struct T(Timestamp, DhtOpHash);

impl AsRef<[u8]> for IntegrationQueueKey {
    fn as_ref(&self) -> &[u8] {
        self.0.bytes().as_ref()
    }
}

impl TryFrom<(Timestamp, DhtOpHash)> for IntegrationQueueKey {
    type Error = SerializedBytesError;
    fn try_from(t: (Timestamp, DhtOpHash)) -> Result<Self, Self::Error> {
        Ok(Self(SerializedBytes::try_from(T(t.0, t.1))?))
    }
}

impl TryFrom<IntegrationQueueKey> for (Timestamp, DhtOpHash) {
    type Error = SerializedBytesError;
    fn try_from(key: IntegrationQueueKey) -> Result<Self, Self::Error> {
        let t = T::try_from(key.0)?;
        Ok((t.0, t.1))
    }
}

/// A type for storing in databases that only need the hashes.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct IntegrationValue {
    /// Thi ops validation status
    pub validation_status: ValidationStatus,
    /// Where to send this op
    pub basis: AnyDhtHash,
    /// Signatures and hashes of the op
    pub op: DhtOpLight,
}

/// A type for storing in databases that only need the hashes.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct IntegrationQueueValue {
    /// Thi ops validation status
    pub validation_status: ValidationStatus,
    /// Signatures and hashes of the op
    pub op: DhtOp,
}

#[cfg(test)]
impl From<SerializedBytes> for IntegrationQueueKey {
    fn from(s: SerializedBytes) -> Self {
        Self(s)
    }
}
