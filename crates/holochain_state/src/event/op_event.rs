use std::collections::BTreeMap;

use derive_more::Constructor;
use kitsune_p2p::dependencies::kitsune_p2p_fetch::TransferMethod;

use crate::{prelude::*, query::map_sql_dht_op};

#[derive(
    Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize,
)]
pub enum OpEvent {
    /// The node has integrated an op authored by someone else
    Integrated { op: DhtOpHash },

    /// The node has app validated an op authored by someone else
    AppValidated { op: DhtOpHash },

    /// The node has sys validated an op authored by someone else
    SysValidated { op: DhtOpHash },

    /// The node has fetched this op from another node via the FetchPool
    Fetched { op: DhtOp },

    /// The node has authored this op, including validation and integration
    Authored { op: DhtOp },
}

#[derive(derive_more::Constructor)]
pub struct OpEventStore {
    authored: DbWrite<DbKindAuthored>,
    dht: DbWrite<DbKindDht>,
}

#[allow(unused)]
impl OpEventStore {
    pub fn new_test(cell_id: CellId) -> Self {
        Self::new(
            test_in_mem_db(DbKindAuthored(cell_id.clone().into())),
            test_in_mem_db(DbKindDht(cell_id.dna_hash().clone().into())),
        )
    }

    pub async fn apply_events(&self, events: Vec<OpEvent>) -> StateMutationResult<()> {
        self.authored
            .write_async(|txn| {
                for event in events {
                    match event {
                        OpEvent::Authored { op } => {
                            insert_op(txn, &op.into_hashed())?;
                        }
                        OpEvent::Integrated { .. } => {
                            unimplemented!("Integrated event not implemented")
                        }
                        OpEvent::AppValidated { .. } => {
                            unimplemented!("AppValidated event not implemented")
                        }
                        OpEvent::SysValidated { .. } => {
                            unimplemented!("SysValidated event not implemented")
                        }
                        OpEvent::Fetched { .. } => unimplemented!("Fetched event not implemented"),
                    }
                }
                Ok(())
            })
            .await
    }

    pub async fn get_events(&self) -> StateQueryResult<Vec<(Timestamp, OpEvent)>> {
        let ops = self
            .authored
            .read_async(|txn| {
                txn.prepare_cached(
                    "
                    SELECT
                    Action.blob as action_blob,
                    Entry.blob as entry_blob,
                    DhtOp.type as dht_type,
                    DhtOp.authored_timestamp
                    FROM Action
                    JOIN
                    DhtOp ON DhtOp.action_hash = Action.hash
                    LEFT JOIN
                    Entry ON Action.entry_hash = Entry.hash
                    ORDER BY DhtOp.authored_timestamp ASC
            ",
                )?
                .query_and_then([], |row| {
                    let timestamp: Timestamp = row.get("authored_timestamp")?;
                    let op = map_sql_dht_op(true, "dht_type", row)?;
                    let authored = OpEvent::Authored { op };
                    StateQueryResult::Ok((timestamp, authored))
                })?
                .collect::<Result<BTreeMap<_, _>, _>>()
            })
            .await?;
        Ok(ops.into_iter().collect::<Vec<_>>())
    }
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeSet, HashSet};

    use super::*;
    use ::fixt::prelude::*;
    use arbitrary::Arbitrary;

    #[tokio::test(flavor = "multi_thread")]
    async fn test_event_creation() {
        let mut u = unstructured_noise();
        let cell_id = fixt!(CellId);
        let op1: DhtOp = ChainOp::arbitrary(&mut u)
            .unwrap()
            .normalized()
            .unwrap()
            .into();
        let op2: DhtOp = ChainOp::arbitrary(&mut u)
            .unwrap()
            .normalized()
            .unwrap()
            .into();
        let op3: DhtOp = ChainOp::arbitrary(&mut u)
            .unwrap()
            .normalized()
            .unwrap()
            .into();

        // let h1 = op1.to_hash();

        let events = maplit::btreeset![
            OpEvent::Authored { op: op1.clone() },
            OpEvent::Authored { op: op2.clone() },
            OpEvent::Authored { op: op3.clone() },
        ];

        let store = OpEventStore::new_test(cell_id);

        store
            .apply_events(events.clone().into_iter().collect())
            .await
            .unwrap();

        let events2: BTreeSet<_> = store
            .get_events()
            .await
            .unwrap()
            .into_iter()
            .map(second)
            .collect();

        pretty_assertions::assert_eq!(events, events2);
    }
}
