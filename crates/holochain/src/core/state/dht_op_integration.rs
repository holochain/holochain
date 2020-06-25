//! Various types for the databases involved in the DhtOp integration workflow

use crate::core::workflow::produce_dht_ops_workflow::dht_op::DhtOpLight;
use fallible_iterator::FallibleIterator;
use holo_hash::DhtOpHash;
use holo_hash_core::HoloHashCoreHash;
use holochain_serialized_bytes::prelude::*;
use holochain_state::{
    buffer::KvBuf,
    db::INTEGRATED_DHT_OPS,
    error::{DatabaseError, DatabaseResult},
    prelude::{BufferedStore, GetDb, Reader},
};
use holochain_types::{
    composite_hash::AnyDhtHash, dht_op::DhtOp, validate::ValidationStatus, Timestamp,
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
pub type IntegratedDhtOpsStore<'env> = KvBuf<'env, DhtOpHash, IntegrationValue, Reader<'env>>;

/// Buffer that adds query logic to the IntegratedDhtOpsStore
pub struct IntegratedDhtOpsBuf<'env> {
    store: IntegratedDhtOpsStore<'env>,
}

impl<'env> std::ops::Deref for IntegratedDhtOpsBuf<'env> {
    type Target = IntegratedDhtOpsStore<'env>;
    fn deref(&self) -> &Self::Target {
        &self.store
    }
}

impl<'env> std::ops::DerefMut for IntegratedDhtOpsBuf<'env> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.store
    }
}

impl<'env> BufferedStore<'env> for IntegratedDhtOpsBuf<'env> {
    type Error = DatabaseError;
    fn flush_to_txn(
        self,
        writer: &'env mut holochain_state::prelude::Writer,
    ) -> Result<(), Self::Error> {
        self.store.flush_to_txn(writer)
    }
}

/// The key type for the IntegrationQueue db
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
#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
pub struct IntegrationValue {
    /// Thi ops validation status
    pub validation_status: ValidationStatus,
    /// Where to send this op
    pub basis: AnyDhtHash,
    /// Signatures and hashes of the op
    pub op: DhtOpLight,
    /// Time when the op was integrated
    pub when_integrated: Timestamp,
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

impl<'env> IntegratedDhtOpsBuf<'env> {
    /// Create a new buffer for the IntegratedDhtOpsStore
    pub fn new(reader: &'env Reader<'env>, dbs: &impl GetDb) -> DatabaseResult<Self> {
        let db = dbs.get_db(&*INTEGRATED_DHT_OPS).unwrap();
        Ok(Self {
            store: IntegratedDhtOpsStore::new(&reader, db)?,
        })
    }
    /// Get ops that match optional queries:
    /// - from a time (Inclusive)
    /// - to a time (Exclusive)
    /// - match a dht location
    pub fn query(
        &'env self,
        from: Option<Timestamp>,
        to: Option<Timestamp>,
        dht_loc: Option<u32>,
    ) -> DatabaseResult<
        Box<dyn FallibleIterator<Item = IntegrationValue, Error = DatabaseError> + 'env>,
    > {
        Ok(Box::new(
            self.store
                .iter()?
                .filter_map(move |(_, v)| match from {
                    Some(time) if v.when_integrated >= time => Ok(Some(v)),
                    None => Ok(Some(v)),
                    _ => Ok(None),
                })
                .filter_map(move |v| match to {
                    Some(time) if v.when_integrated < time => Ok(Some(v)),
                    None => Ok(Some(v)),
                    _ => Ok(None),
                })
                .filter_map(move |v| match dht_loc {
                    Some(dht_loc) if v.basis.get_loc() == dht_loc => Ok(Some(v)),
                    None => Ok(Some(v)),
                    _ => Ok(None),
                }),
        ))
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixt::AnyDhtHashFixturator;
    use chrono::{Duration, Utc};
    use fixt::prelude::*;
    use holo_hash::{DhtOpHashFixturator, HeaderHashFixturator};
    use holochain_state::test_utils::test_cell_env;
    use holochain_state::{
        buffer::BufferedStore,
        env::{ReadManager, WriteManager},
    };
    use holochain_types::fixt::SignatureFixturator;
    use pretty_assertions::assert_eq;

    #[tokio::test(threaded_scheduler)]
    async fn test_query() {
        let env = test_cell_env();
        let dbs = env.dbs().await;
        let env_ref = env.guard().await;

        // Create some integration valuesA
        let mut expected = Vec::new();
        let mut basis = AnyDhtHashFixturator::new(Predictable);
        let now = Utc::now();
        let same_basis = basis.next().unwrap();
        let mut times = Vec::new();
        times.push(now - Duration::hours(100));
        times.push(now);
        times.push(now + Duration::hours(100));
        let times_exp = times.clone();
        let values = times.into_iter().map(|when_integrated| IntegrationValue {
            validation_status: ValidationStatus::Valid,
            basis: basis.next().unwrap(),
            op: DhtOpLight::RegisterAgentActivity(fixt!(Signature), fixt!(HeaderHash)),
            when_integrated: when_integrated.into(),
        });

        // Put them in the db
        {
            let mut dht_hash = DhtOpHashFixturator::new(Predictable);
            let reader = env_ref.reader().unwrap();
            let mut buf = IntegratedDhtOpsBuf::new(&reader, &dbs).unwrap();
            for mut value in values {
                buf.put(dht_hash.next().unwrap(), value.clone()).unwrap();
                expected.push(value.clone());
                value.basis = same_basis.clone();
                buf.put(dht_hash.next().unwrap(), value.clone()).unwrap();
                expected.push(value.clone());
            }
            env_ref
                .with_commit(|writer| buf.flush_to_txn(writer))
                .unwrap();
        }

        // Check queries
        {
            let reader = env_ref.reader().unwrap();
            let buf = IntegratedDhtOpsBuf::new(&reader, &dbs).unwrap();
            // No filter
            let mut r = buf
                .query(None, None, None)
                .unwrap()
                .collect::<Vec<_>>()
                .unwrap();
            r.sort_by_key(|v| v.when_integrated.clone());
            assert_eq!(&r[..], &expected[..]);
            // From now
            let mut r = buf
                .query(Some(times_exp[1].clone().into()), None, None)
                .unwrap()
                .collect::<Vec<_>>()
                .unwrap();
            r.sort_by_key(|v| v.when_integrated.clone());
            assert!(r.contains(&expected[2]));
            assert!(r.contains(&expected[4]));
            assert!(r.contains(&expected[3]));
            assert!(r.contains(&expected[5]));
            assert_eq!(r.len(), 4);
            // From ages ago till 1hr in future
            let ages_ago = times_exp[0] - Duration::weeks(5);
            let future = times_exp[1] + Duration::hours(1);
            let mut r = buf
                .query(Some(ages_ago.into()), Some(future.into()), None)
                .unwrap()
                .collect::<Vec<_>>()
                .unwrap();
            r.sort_by_key(|v| v.when_integrated.clone());

            assert!(r.contains(&expected[0]));
            assert!(r.contains(&expected[1]));
            assert!(r.contains(&expected[2]));
            assert!(r.contains(&expected[3]));
            assert_eq!(r.len(), 4);
            // Same basis
            let ages_ago = times_exp[0] - Duration::weeks(5);
            let future = times_exp[1] + Duration::hours(1);
            let mut r = buf
                .query(
                    Some(ages_ago.into()),
                    Some(future.into()),
                    Some(same_basis.get_loc()),
                )
                .unwrap()
                .collect::<Vec<_>>()
                .unwrap();
            r.sort_by_key(|v| v.when_integrated.clone());
            assert!(r.contains(&expected[1]));
            assert!(r.contains(&expected[3]));
            assert_eq!(r.len(), 2);
            // Same basis all
            let mut r = buf
                .query(None, None, Some(same_basis.get_loc()))
                .unwrap()
                .collect::<Vec<_>>()
                .unwrap();
            r.sort_by_key(|v| v.when_integrated.clone());
            assert!(r.contains(&expected[1]));
            assert!(r.contains(&expected[3]));
            assert!(r.contains(&expected[5]));
            assert_eq!(r.len(), 3);
        }
    }
}
