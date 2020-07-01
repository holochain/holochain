//! Various types for the databases involved in the DhtOp integration workflow

use crate::core::workflow::produce_dht_ops_workflow::dht_op_light::DhtOpLight;
use holo_hash::DhtOpHash;
use holochain_serialized_bytes::prelude::*;
use holochain_state::{buffer::KvBuf, prelude::Reader};
use holochain_types::{
    composite_hash::AnyDhtHash, dht_op::DhtOp, validate::ValidationStatus, Timestamp, TimestampKey,
};

/// Database type for AuthoredDhtOps
/// Buffer for accessing [DhtOp]s that you authored and finding the amount of validation receipts
pub type AuthoredDhtOpsStore<'env> = KvBuf<'env, DhtOpHash, u32, Reader<'env>>;

/// Database type for IntegrationQueue
/// Queue of ops ready to be integrated
pub type IntegrationQueueStore<'env> =
    KvBuf<'env, IntegrationQueueKey, IntegrationQueueValue, Reader<'env>>;

/// Database type for IntegratedDhtOps
/// [DhtOp]s that have already been integrated
pub type IntegratedDhtOpsStore<'env> = KvBuf<'env, DhtOpHash, IntegratedDhtOpsValue, Reader<'env>>;

/// The key type for the IntegrationQueue db
/// Key for the Integration Queue that ensures they are ordered by time
#[derive(Hash, Eq, PartialEq)]
pub struct IntegrationQueueKey(SerializedBytes);

/// The key type for the IntegrationQueue db.
/// It is carefully constructed to ensure that keys are properly ordered by
/// timestamp and will also be unique
#[cfg_attr(test, derive(Hash, Eq, PartialEq, PartialOrd, Ord))]
pub struct IQK(Vec<u8>);

impl AsRef<[u8]> for IQK {
    fn as_ref(&self) -> &[u8] {
        self.0.as_ref()
    }
}

impl TryFrom<(TimestampKey, DhtOpHash)> for IQK {
    type Error = SerializedBytesError;
    fn try_from((timestamp, hash): (TimestampKey, DhtOpHash)) -> Result<Self, Self::Error> {
        Ok(Self(
            bincode::serialize(&(timestamp.as_ref(), hash)).unwrap(),
        ))
    }
}

impl TryFrom<&IQK> for (TimestampKey, DhtOpHash) {
    type Error = SerializedBytesError;
    fn try_from(k: &IQK) -> Result<Self, Self::Error> {
        let slice: &[u8] = k.0.as_ref();
        let (bytes, hash): (Vec<u8>, DhtOpHash) = bincode::deserialize(slice).unwrap();
        Ok((TimestampKey::from(bytes.as_ref()), hash))
    }
}

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
#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
pub struct IntegratedDhtOpsValue {
    /// The op's validation status
    pub validation_status: ValidationStatus,
    /// Where to send this op
    pub basis: AnyDhtHash,
    /// Signatures and hashes of the op
    pub op: DhtOpLight,
}

/// A type for storing in databases that only need the hashes.
#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
pub struct IntegrationQueueValue {
    /// The op's validation status
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

#[cfg(test)]
mod tests {
    use super::*;
    use holo_hash::{DhtOpHash, HoloHashBaseExt};

    #[test]
    fn test_integration_queue_key_roundtrips() {
        // create test timestamps
        let ts1 = Timestamp(i64::MIN, u32::MIN);
        let ts2 = Timestamp(i64::MIN / 4, u32::MAX);
        let ts3 = Timestamp::try_from("1930-01-01T00:00:00.999999999Z").unwrap();
        let ts4 = Timestamp::try_from("1970-11-11T14:34:00.000000000Z").unwrap();
        let ts5 = Timestamp::try_from("2020-05-05T19:16:04.266431045Z").unwrap();
        let ts6 = Timestamp(i64::MAX / 4, u32::MIN);
        let ts7 = Timestamp(i64::MAX, u32::MAX);

        // build corresponding timestamp keys
        let tk1 = TimestampKey::from(ts1.clone());
        let tk2 = TimestampKey::from(ts2.clone());
        let tk3 = TimestampKey::from(ts3.clone());
        let tk4 = TimestampKey::from(ts4.clone());
        let tk5 = TimestampKey::from(ts5.clone());
        let tk6 = TimestampKey::from(ts6.clone());
        let tk7 = TimestampKey::from(ts7.clone());

        // dummy hash
        let h = DhtOpHash::with_pre_hashed([0; 32].to_vec());

        // create tuples to check roundtrips
        let p1 = (tk1, h.clone());
        let p2 = (tk2, h.clone());
        let p3 = (tk3, h.clone());
        let p4 = (tk4, h.clone());
        let p5 = (tk5, h.clone());
        let p6 = (tk6, h.clone());
        let p7 = (tk7, h.clone());

        // tuple -> bytes
        let ik1 = IQK::try_from(p1.clone()).unwrap();
        let ik2 = IQK::try_from(p2.clone()).unwrap();
        let ik3 = IQK::try_from(p3.clone()).unwrap();
        let ik4 = IQK::try_from(p4.clone()).unwrap();
        let ik5 = IQK::try_from(p5.clone()).unwrap();
        let ik6 = IQK::try_from(p6.clone()).unwrap();
        let ik7 = IQK::try_from(p7.clone()).unwrap();

        // bytes -> tuple
        assert_eq!(p1, (&ik1).try_into().unwrap());
        assert_eq!(p2, (&ik2).try_into().unwrap());
        assert_eq!(p3, (&ik3).try_into().unwrap());
        assert_eq!(p4, (&ik4).try_into().unwrap());
        assert_eq!(p5, (&ik5).try_into().unwrap());
        assert_eq!(p6, (&ik6).try_into().unwrap());
        assert_eq!(p7, (&ik7).try_into().unwrap());

        // test absolute ordering
        assert!(ik1 < ik2);
        assert!(ik2 < ik3);
        assert!(ik3 < ik4);
        assert!(ik4 < ik5);
        assert!(ik5 < ik6);
        assert!(ik6 < ik7);
    }
}
