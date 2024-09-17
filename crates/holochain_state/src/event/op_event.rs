//! This module defines the two-way mapping between OpEvents and the DhtOp databases.
//!
//! TODO:
//! - [ ] Define what happens when an op is stored in both Authored and DHT databases,
//!     potentially at different times and stages of integratation.

use std::collections::{BTreeMap, HashMap};

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

    pub async fn apply_event(&self, event: OpEvent) -> StateMutationResult<()> {
        self.apply_events(vec![event]).await
    }

    pub async fn apply_events(&self, events: Vec<OpEvent>) -> StateMutationResult<()> {
        enum Db {
            Authored,
            Dht,
        }
        let mut ops = HashMap::new();
        for event in events {
            match event {
                OpEvent::Authored { op } => {
                    let op = op.into_hashed();
                    ops.insert(op.as_hash().clone(), (op.clone(), Db::Authored));
                    self.authored
                        .write_async(move |txn| insert_op(txn, &op))
                        .await?;
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
                OpEvent::Fetched { op } => {
                    let op = op.into_hashed();
                    ops.insert(op.as_hash().clone(), (op.clone(), Db::Dht));
                    self.dht.write_async(move |txn| insert_op(txn, &op)).await?;
                }
            }
        }

        Ok(())
    }

    pub async fn get_events(&self) -> StateQueryResult<Vec<(Timestamp, OpEvent)>> {
        let sql_common = "
            SELECT
            Action.blob as action_blob,
            Entry.blob as entry_blob,
            DhtOp.type as dht_type,
            DhtOp.authored_timestamp,
            
            DhtOp.when_sys_validated,
            DhtOp.when_app_validated,
            DhtOp.when_integrated
            FROM Action
            JOIN
            DhtOp ON DhtOp.action_hash = Action.hash
            LEFT JOIN
            Entry ON Action.entry_hash = Entry.hash
            ORDER BY DhtOp.authored_timestamp ASC
        ";
        let events_authored = self
            .authored
            .read_async(|txn| {
                txn.prepare_cached(sql_common)?
                    .query_and_then([], |row| {
                        let timestamp: Timestamp = row.get("authored_timestamp")?;
                        let op = map_sql_dht_op(true, "dht_type", row)?;
                        let op_hash = op.to_hash();

                        // The existence of an op implies the Authored event
                        let mut events = vec![(timestamp, OpEvent::Authored { op })];

                        // The existence of a when_sys_validated timestamp
                        // implies the SysValidated event
                        if let Some(when_sys_validated) = row.get("when_sys_validated")? {
                            let ev = OpEvent::SysValidated {
                                op: op_hash.clone(),
                            };
                            events.push((when_sys_validated, ev));
                        }

                        // The existence of a when_app_validated timestamp
                        // implies the AppValidated event
                        if let Some(when_app_validated) = row.get("when_app_validated")? {
                            let ev = OpEvent::AppValidated {
                                op: op_hash.clone(),
                            };
                            events.push((when_app_validated, ev));
                        }

                        // The existence of a when_integrated timestamp
                        // implies the Integrated event
                        if let Some(when_integrated) = row.get("when_integrated")? {
                            let ev = OpEvent::Integrated {
                                op: op_hash.clone(),
                            };
                            events.push((when_integrated, ev));
                        }

                        StateQueryResult::Ok(events)
                    })?
                    .collect::<Result<Vec<Vec<_>>, _>>()
            })
            .await?;

        let events_dht = self
            .dht
            .read_async(|txn| {
                txn.prepare_cached(sql_common)?
                    .query_and_then([], |row| {
                        let timestamp: Timestamp = row.get("authored_timestamp")?;
                        let op = map_sql_dht_op(true, "dht_type", row)?;
                        let op_hash = op.to_hash();

                        // The existence of an op implies the Fetched event
                        let mut events = vec![(timestamp, OpEvent::Fetched { op })];

                        // The existence of a when_sys_validated timestamp
                        // implies the SysValidated event
                        if let Some(when_sys_validated) = row.get("when_sys_validated")? {
                            let ev = OpEvent::SysValidated {
                                op: op_hash.clone(),
                            };
                            events.push((when_sys_validated, ev));
                        }

                        // The existence of a when_app_validated timestamp
                        // implies the AppValidated event
                        if let Some(when_app_validated) = row.get("when_app_validated")? {
                            let ev = OpEvent::AppValidated {
                                op: op_hash.clone(),
                            };
                            events.push((when_app_validated, ev));
                        }

                        // The existence of a when_integrated timestamp
                        // implies the Integrated event
                        if let Some(when_integrated) = row.get("when_integrated")? {
                            let ev = OpEvent::Integrated {
                                op: op_hash.clone(),
                            };
                            events.push((when_integrated, ev));
                        }

                        StateQueryResult::Ok(events)
                    })?
                    .collect::<Result<Vec<Vec<_>>, _>>()
            })
            .await?;

        Ok(events_authored
            .into_iter()
            .chain(events_dht.into_iter())
            .flatten()
            .collect::<BTreeMap<_, _>>()
            .into_iter()
            .collect::<Vec<_>>())
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::*;
    use ::fixt::prelude::*;
    use arbitrary::Arbitrary;

    async fn db_roundtrip(
        store: &mut OpEventStore,
        events: &BTreeSet<OpEvent>,
    ) -> BTreeSet<OpEvent> {
        store
            .apply_events(events.clone().into_iter().collect())
            .await
            .unwrap();

        store
            .get_events()
            .await
            .unwrap()
            .into_iter()
            .map(second)
            .collect()
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_event_creation() {
        const NUM_OPS: usize = 3;
        let mut u = unstructured_noise();
        let ops: Vec<DhtOp> = (0..NUM_OPS)
            .map(|_| {
                ChainOp::arbitrary(&mut u)
                    .unwrap()
                    .normalized()
                    .unwrap()
                    .into()
            })
            .collect();

        let cell_id_1 = fixt!(CellId);
        let cell_id_2 = CellId::new(cell_id_1.dna_hash().clone(), fixt!(AgentPubKey));

        // Setup store 1

        let events_1 = maplit::btreeset![
            OpEvent::Authored { op: ops[0].clone() },
            OpEvent::Authored { op: ops[1].clone() },
            OpEvent::Authored { op: ops[2].clone() },
        ];
        let mut store_1 = OpEventStore::new_test(cell_id_1);
        let extracted_events_1 = db_roundtrip(&mut store_1, &events_1).await;
        pretty_assertions::assert_eq!(events_1, extracted_events_1);

        // Setup store 2

        let events_2 = maplit::btreeset![
            OpEvent::Fetched { op: ops[0].clone() },
            OpEvent::Fetched { op: ops[1].clone() },
            OpEvent::Fetched { op: ops[2].clone() },
        ];
        let mut store_2 = OpEventStore::new_test(cell_id_2);
        let extracted_events_2 = db_roundtrip(&mut store_2, &events_2).await;
        pretty_assertions::assert_eq!(events_2, extracted_events_2);
    }
}
