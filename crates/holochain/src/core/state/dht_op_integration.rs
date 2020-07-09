//! Various types for the databases involved in the DhtOp integration workflow

use crate::core::workflow::produce_dht_ops_workflow::dht_op_light::DhtOpLight;
use holo_hash::DhtOpHash;
use holo_hash::HoloHashBaseExt;
use holochain_serialized_bytes::prelude::*;
use holochain_state::{buffer::KvBuf, prelude::Reader};
use holochain_types::{
    composite_hash::AnyDhtHash, dht_op::DhtOp, timestamp::TS_SIZE, validate::ValidationStatus,
    TimestampKey,
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

/// The key type for the IntegrationQueue db.
/// It is carefully constructed to ensure that keys are properly ordered by
/// timestamp and will also be unique
#[derive(Hash, PartialEq, Eq)]
#[cfg_attr(test, derive(PartialOrd, Ord))]
pub struct IntegrationQueueKey(Vec<u8>);

impl AsRef<[u8]> for IntegrationQueueKey {
    fn as_ref(&self) -> &[u8] {
        self.0.as_ref()
    }
}

impl From<&[u8]> for IntegrationQueueKey {
    fn from(slice: &[u8]) -> Self {
        Self(slice.to_vec())
    }
}

impl From<(TimestampKey, DhtOpHash)> for IntegrationQueueKey {
    fn from((timestamp, hash): (TimestampKey, DhtOpHash)) -> Self {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(timestamp.as_ref());
        bytes.extend_from_slice(hash.as_ref());
        Self(bytes)
    }
}

impl From<&IntegrationQueueKey> for (TimestampKey, DhtOpHash) {
    fn from(k: &IntegrationQueueKey) -> Self {
        let slice: &[u8] = k.0.as_ref();
        let ts = &slice[0..TS_SIZE];
        let hash = &slice[TS_SIZE..];
        (
            TimestampKey::from(ts),
            DhtOpHash::with_pre_hashed(hash.to_vec()),
        )
    }
}

impl From<IntegrationQueueKey> for (TimestampKey, DhtOpHash) {
    fn from(k: IntegrationQueueKey) -> Self {
        (&k).into()
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
mod tests {
    use super::*;
    use holo_hash::{DhtOpHash, HoloHashBaseExt};
    use holochain_types::Timestamp;

    #[test]
    fn test_integration_queue_key_roundtrips() {
        // create test timestamps
        let ts1 = Timestamp(i64::MIN, u32::MIN);
        let ts3 = Timestamp::try_from("1930-01-01T00:00:00.999999999Z").unwrap();
        let ts5 = Timestamp::try_from("2020-05-05T19:16:04.266431045Z").unwrap();
        let ts7 = Timestamp(i64::MAX, u32::MAX);

        // build corresponding timestamp keys
        let tk1 = TimestampKey::from(ts1.clone());
        let tk3 = TimestampKey::from(ts3.clone());
        let tk5 = TimestampKey::from(ts5.clone());
        let tk7 = TimestampKey::from(ts7.clone());

        // dummy hash
        let h0 = DhtOpHash::with_pre_hashed([0; 32].to_vec());
        let h1 = DhtOpHash::with_pre_hashed([1; 32].to_vec());

        // create tuples to check roundtrips
        let p1 = (tk1, h0.clone());
        let p2 = (tk1, h1.clone());
        let p3 = (tk3, h0.clone());
        let p4 = (tk3, h1.clone());
        let p5 = (tk5, h0.clone());
        let p6 = (tk5, h1.clone());
        let p7 = (tk7, h0.clone());
        let p8 = (tk7, h1.clone());

        // tuple -> bytes
        let ik1 = IntegrationQueueKey::try_from(p1.clone()).unwrap();
        let ik2 = IntegrationQueueKey::try_from(p2.clone()).unwrap();
        let ik3 = IntegrationQueueKey::try_from(p3.clone()).unwrap();
        let ik4 = IntegrationQueueKey::try_from(p4.clone()).unwrap();
        let ik5 = IntegrationQueueKey::try_from(p5.clone()).unwrap();
        let ik6 = IntegrationQueueKey::try_from(p6.clone()).unwrap();
        let ik7 = IntegrationQueueKey::try_from(p7.clone()).unwrap();
        let ik8 = IntegrationQueueKey::try_from(p8.clone()).unwrap();

        // bytes -> tuple
        assert_eq!(p1, (&ik1).try_into().unwrap());
        assert_eq!(p2, (&ik2).try_into().unwrap());
        assert_eq!(p3, (&ik3).try_into().unwrap());
        assert_eq!(p4, (&ik4).try_into().unwrap());
        assert_eq!(p5, (&ik5).try_into().unwrap());
        assert_eq!(p6, (&ik6).try_into().unwrap());
        assert_eq!(p7, (&ik7).try_into().unwrap());
        assert_eq!(p8, (&ik8).try_into().unwrap());

        // test absolute ordering
        assert!(ik1 < ik2);
        assert!(ik2 < ik3);
        assert!(ik3 < ik4);
        assert!(ik4 < ik5);
        assert!(ik5 < ik6);
        assert!(ik6 < ik7);
        assert!(ik7 < ik8);
    }
}
