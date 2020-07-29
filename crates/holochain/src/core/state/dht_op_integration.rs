//! Various types for the databases involved in the DhtOp integration workflow

use fallible_iterator::FallibleIterator;
use holo_hash::*;
use holochain_p2p::dht_arc::DhtArc;
use holochain_serialized_bytes::prelude::*;
use holochain_state::{
    buffer::KvBuf,
    db::INTEGRATED_DHT_OPS,
    error::{DatabaseError, DatabaseResult},
    prelude::{BufferedStore, GetDb, Reader},
};
use holochain_types::{
    dht_op::{DhtOp, DhtOpLight},
    timestamp::TS_SIZE,
    validate::ValidationStatus,
    Timestamp, TimestampKey,
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
    /// Signatures and hashes of the op
    pub op: DhtOpLight,
    /// Time when the op was integrated
    pub when_integrated: Timestamp,
}

/// A type for storing in databases that only need the hashes.
#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
pub struct IntegrationQueueValue {
    /// The op's validation status
    pub validation_status: ValidationStatus,
    /// Signatures and hashes of the op
    pub op: DhtOp,
}

impl<'env> IntegratedDhtOpsBuf<'env> {
    /// Create a new buffer for the IntegratedDhtOpsStore
    pub fn new(reader: &'env Reader<'env>, dbs: &impl GetDb) -> DatabaseResult<Self> {
        let db = dbs.get_db(&*INTEGRATED_DHT_OPS).unwrap();
        Ok(Self {
            store: IntegratedDhtOpsStore::new(&reader, db)?,
        })
    }

    /// simple get by dht_op_hash
    pub fn get(&'env self, op_hash: &DhtOpHash) -> DatabaseResult<Option<IntegratedDhtOpsValue>> {
        self.store.get(op_hash)
    }

    /// Get ops that match optional queries:
    /// - from a time (Inclusive)
    /// - to a time (Exclusive)
    /// - match a dht location
    pub fn query(
        &'env self,
        from: Option<Timestamp>,
        to: Option<Timestamp>,
        dht_arc: Option<DhtArc>,
    ) -> DatabaseResult<
        Box<
            dyn FallibleIterator<Item = (DhtOpHash, IntegratedDhtOpsValue), Error = DatabaseError>
                + 'env,
        >,
    > {
        Ok(Box::new(
            self.store
                .iter()?
                .map(move |(k, v)| Ok((DhtOpHash::with_pre_hashed(k.to_vec()), v)))
                .filter_map(move |(k, v)| match from {
                    Some(time) if v.when_integrated >= time => Ok(Some((k, v))),
                    None => Ok(Some((k, v))),
                    _ => Ok(None),
                })
                .filter_map(move |(k, v)| match to {
                    Some(time) if v.when_integrated < time => Ok(Some((k, v))),
                    None => Ok(Some((k, v))),
                    _ => Ok(None),
                })
                .filter_map(move |(k, v)| match dht_arc {
                    Some(dht_arc) if dht_arc.contains(v.op.dht_basis().get_loc()) => {
                        Ok(Some((k, v)))
                    }
                    None => Ok(Some((k, v))),
                    _ => Ok(None),
                }),
        ))
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixt::AnyDhtHashFixturator;
    use ::fixt::prelude::*;
    use chrono::{Duration, Utc};
    use holo_hash::fixt::{DhtOpHashFixturator, HeaderHashFixturator};
    use holochain_state::test_utils::test_cell_env;
    use holochain_state::{
        buffer::BufferedStore,
        env::{ReadManager, WriteManager},
    };
    use holochain_types::Timestamp;
    use pretty_assertions::assert_eq;

    #[tokio::test(threaded_scheduler)]
    async fn test_query() {
        let env = test_cell_env();
        let dbs = env.dbs().await;
        let env_ref = env.guard().await;

        // Create some integration values
        let mut expected = Vec::new();
        let mut basis = AnyDhtHashFixturator::new(Predictable);
        let now = Utc::now();
        let same_basis = basis.next().unwrap();
        let mut times = Vec::new();
        times.push(now - Duration::hours(100));
        times.push(now);
        times.push(now + Duration::hours(100));
        let times_exp = times.clone();
        let values = times
            .into_iter()
            .map(|when_integrated| IntegratedDhtOpsValue {
                validation_status: ValidationStatus::Valid,
                op: DhtOpLight::RegisterAgentActivity(fixt!(HeaderHash), basis.next().unwrap()),
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
                value.op = DhtOpLight::RegisterAgentActivity(fixt!(HeaderHash), same_basis.clone());
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
                .map(|(_, v)| Ok(v))
                .collect::<Vec<_>>()
                .unwrap();
            r.sort_by_key(|v| v.when_integrated.clone());
            assert_eq!(&r[..], &expected[..]);
            // From now
            let mut r = buf
                .query(Some(times_exp[1].clone().into()), None, None)
                .unwrap()
                .map(|(_, v)| Ok(v))
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
                .map(|(_, v)| Ok(v))
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
                    Some(DhtArc::new(same_basis.get_loc(), 1)),
                )
                .unwrap()
                .map(|(_, v)| Ok(v))
                .collect::<Vec<_>>()
                .unwrap();
            r.sort_by_key(|v| v.when_integrated.clone());
            assert!(r.contains(&expected[1]));
            assert!(r.contains(&expected[3]));
            assert_eq!(r.len(), 2);
            // Same basis all
            let mut r = buf
                .query(None, None, Some(DhtArc::new(same_basis.get_loc(), 1)))
                .unwrap()
                .map(|(_, v)| Ok(v))
                .collect::<Vec<_>>()
                .unwrap();
            r.sort_by_key(|v| v.when_integrated.clone());
            assert!(r.contains(&expected[1]));
            assert!(r.contains(&expected[3]));
            assert!(r.contains(&expected[5]));
            assert_eq!(r.len(), 3);
        }
    }

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
