use crate::core::workflow::produce_dht_ops_workflow::dht_op::DhtOpLight;
use holo_hash::DhtOpHash;
use holochain_serialized_bytes::prelude::*;
use holochain_state::{buffer::KvBuf, prelude::Reader};
use holochain_types::{composite_hash::AnyDhtHash, validate::ValidationStatus, Timestamp};

pub type AuthoredDhtOps<'env> = KvBuf<'env, DhtOpHash, u32, Reader<'env>>;

pub type IntegrationQueue<'env> = KvBuf<'env, IntegrationQueueKey, IntegrationValue, Reader<'env>>;

pub type IntegratedDhtOps<'env> = KvBuf<'env, DhtOpHash, IntegrationValue, Reader<'env>>;

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

#[cfg(test)]
impl From<SerializedBytes> for IntegrationQueueKey {
    fn from(s: SerializedBytes) -> Self {
        Self(s)
    }
}
