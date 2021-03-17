//! Various types for the databases involved in the DhtOp integration workflow

use fallible_iterator::FallibleIterator;
use holo_hash::*;
use holochain_p2p::dht_arc::DhtArc;
use holochain_serialized_bytes::prelude::*;
use holochain_sqlite::buffer::KvBufFresh;
use holochain_sqlite::error::DatabaseError;
use holochain_sqlite::error::DatabaseResult;
use holochain_sqlite::prelude::BufferedStore;
use holochain_sqlite::prelude::DbRead;
use holochain_sqlite::prelude::GetTable;
use holochain_sqlite::prelude::Readable;
use holochain_sqlite::prelude::*;
use holochain_types::prelude::*;
use holochain_zome_types::validate::ValidationStatus;

/// Database type for AuthoredDhtOps
/// Buffer for accessing [DhtOp]s that you authored and finding the amount of validation receipts
pub type AuthoredDhtOpsStore = KvBufFresh<AuthoredDhtOpsKey, AuthoredDhtOpsValue>;

/// The key type for the AuthoredDhtOps db: a DhtOpHash
pub type AuthoredDhtOpsKey = DhtOpHash;

/// A type for storing in databases that only need the hashes.
#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
pub struct AuthoredDhtOpsValue {
    /// Signatures and hashes of the op
    pub op: DhtOpLight,
    /// Validation receipts received
    pub receipt_count: u32,
    /// Time last published, None if never published
    pub last_publish_time: Option<Timestamp>,
}

impl AuthoredDhtOpsValue {
    /// Create a new value from a DhtOpLight with no receipts and no timestamp
    pub fn from_light(op: DhtOpLight) -> Self {
        Self {
            op,
            receipt_count: 0,
            last_publish_time: None,
        }
    }
}

/// Database type for IntegrationLimbo: the queue of ops ready to be integrated.
pub type IntegrationLimboStore = KvBufFresh<IntegrationLimboKey, IntegrationLimboValue>;

/// Database type for IntegratedDhtOps
/// [DhtOp]s that have already been integrated
pub type IntegratedDhtOpsStore = KvBufFresh<DhtOpHash, IntegratedDhtOpsValue>;

/// Buffer that adds query logic to the IntegratedDhtOpsStore
pub struct IntegratedDhtOpsBuf {
    store: IntegratedDhtOpsStore,
}

impl std::ops::Deref for IntegratedDhtOpsBuf {
    type Target = IntegratedDhtOpsStore;
    fn deref(&self) -> &Self::Target {
        &self.store
    }
}

impl std::ops::DerefMut for IntegratedDhtOpsBuf {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.store
    }
}

impl BufferedStore for IntegratedDhtOpsBuf {
    type Error = DatabaseError;
    fn flush_to_txn_ref(
        &mut self,
        writer: &mut holochain_sqlite::prelude::Writer,
    ) -> Result<(), Self::Error> {
        self.store.flush_to_txn_ref(writer)
    }
}

/// The key type for the IntegrationLimbo db is just a DhtOpHash
pub type IntegrationLimboKey = DhtOpHash;

/// A type for storing in databases that only need the hashes.
#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
pub struct IntegratedDhtOpsValue {
    /// The op's validation status
    pub validation_status: ValidationStatus,
    /// Signatures and hashes of the op
    pub op: DhtOpLight,
    /// Time when the op was integrated
    pub when_integrated: Timestamp,
    /// Send a receipt to this author.
    pub send_receipt: bool,
}

/// A type for storing in databases that only need the hashes.
#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
pub struct IntegrationLimboValue {
    /// The op's validation status
    pub validation_status: ValidationStatus,
    /// The op
    pub op: DhtOpLight,
    /// Send a receipt to this author.
    pub send_receipt: bool,
}

impl IntegratedDhtOpsBuf {
    /// Create a new buffer for the IntegratedDhtOpsStore
    pub fn new(env: DbRead) -> DatabaseResult<Self> {
        let db = env.get_table(TableName::IntegratedDhtOps).unwrap();
        Ok(Self {
            store: IntegratedDhtOpsStore::new(env, db),
        })
    }

    /// simple get by dht_op_hash
    pub fn get(&'_ self, op_hash: &DhtOpHash) -> DatabaseResult<Option<IntegratedDhtOpsValue>> {
        self.store.get(op_hash)
    }

    /// Get ops that match optional queries:
    /// - from a time (Inclusive)
    /// - to a time (Exclusive)
    /// - match a dht location
    pub fn query<'r, R: Readable>(
        &'r self,
        r: &'r mut R,
        from: Option<Timestamp>,
        to: Option<Timestamp>,
        dht_arc: Option<DhtArc>,
    ) -> DatabaseResult<
        Box<
            dyn FallibleIterator<Item = (DhtOpHash, IntegratedDhtOpsValue), Error = DatabaseError>
                + 'r,
        >,
    > {
        Ok(Box::new(
            self.store
                .iter(r)?
                .map(move |(k, v)| Ok((DhtOpHash::from_raw_39_panicky(k.to_vec()), v)))
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
    use ::fixt::prelude::*;
    use chrono::Duration;
    use chrono::Utc;
    use holo_hash::fixt::AnyDhtHashFixturator;
    use holo_hash::fixt::DhtOpHashFixturator;
    use holo_hash::fixt::HeaderHashFixturator;
    use holochain_sqlite::buffer::BufferedStore;
    use holochain_sqlite::db::ReadManager;
    use holochain_sqlite::db::WriteManager;
    use holochain_sqlite::test_utils::test_cell_env;
    use pretty_assertions::assert_eq;

    #[tokio::test(flavor = "multi_thread")]
    async fn test_query() {
        let test_env = test_cell_env();
        let env = test_env.env();

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
                send_receipt: false,
            });

        // Put them in the db
        {
            let mut dht_hash = DhtOpHashFixturator::new(Predictable);
            let mut buf = IntegratedDhtOpsBuf::new(env.clone().into()).unwrap();
            for mut value in values {
                buf.put(dht_hash.next().unwrap(), value.clone()).unwrap();
                expected.push(value.clone());
                value.op = DhtOpLight::RegisterAgentActivity(fixt!(HeaderHash), same_basis.clone());
                buf.put(dht_hash.next().unwrap(), value.clone()).unwrap();
                expected.push(value.clone());
            }
            env.conn()
                .unwrap()
                .with_commit(|writer| buf.flush_to_txn(writer))
                .unwrap();
        }

        // Check queries

        let mut conn = env.conn().unwrap();
        conn.with_reader_test(|mut reader| {
            let buf = IntegratedDhtOpsBuf::new(env.clone().into()).unwrap();
            // No filter
            let mut r = buf
                .query(&mut reader, None, None, None)
                .unwrap()
                .map(|(_, v)| Ok(v))
                .collect::<Vec<_>>()
                .unwrap();
            r.sort_by_key(|v| v.when_integrated.clone());
            assert_eq!(&mut r[..], &expected[..]);
            // From now
            let mut r = buf
                .query(&mut reader, Some(times_exp[1].clone().into()), None, None)
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
                .query(
                    &mut reader,
                    Some(ages_ago.into()),
                    Some(future.into()),
                    None,
                )
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
                    &mut reader,
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
                .query(
                    &mut reader,
                    None,
                    None,
                    Some(DhtArc::new(same_basis.get_loc(), 1)),
                )
                .unwrap()
                .map(|(_, v)| Ok(v))
                .collect::<Vec<_>>()
                .unwrap();
            r.sort_by_key(|v| v.when_integrated.clone());
            assert!(r.contains(&expected[1]));
            assert!(r.contains(&expected[3]));
            assert!(r.contains(&expected[5]));
            assert_eq!(r.len(), 3);
        });
    }
}
