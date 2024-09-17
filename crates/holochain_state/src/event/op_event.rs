//! This module defines the two-way mapping between OpEvents and the DhtOp databases.
//!
//! TODO:
//! - [ ] Define what happens when an op is stored in both Authored and DHT databases,
//!     potentially at different times and stages of integratation.

use std::collections::HashMap;

use kitsune_p2p::dependencies::kitsune_p2p_fetch::TransferMethod;

use crate::{
    event::EventError,
    prelude::*,
    query::{get_public_op_from_db, map_sql_dht_op},
};

use super::{Event, EventData, EventResult};

#[derive(Clone, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum OpEvent {
    /// The node has authored this op, including validation and integration
    Authored { op: DhtOp },

    /// The node has fetched this op from another node via the FetchPool
    Fetched { op: DhtOp },

    /// The node has sys validated an op authored by someone else
    SysValidated { op: DhtOpHash },

    /// The node has app validated an op authored by someone else
    AppValidated { op: DhtOpHash },

    /// The node has integrated an op authored by someone else
    Integrated { op: DhtOpHash },
}

#[derive(derive_more::Constructor)]
pub struct OpEventStore {
    authored: DbWrite<DbKindAuthored>,
    dht: DbWrite<DbKindDht>,
}

#[derive(Debug, Clone, Copy)]
enum Db {
    Authored,
    Dht,
}

#[allow(unused)]
impl OpEventStore {
    pub fn new_test(cell_id: CellId) -> Self {
        Self::new(
            test_in_mem_db(DbKindAuthored(cell_id.clone().into())),
            test_in_mem_db(DbKindDht(cell_id.dna_hash().clone().into())),
        )
    }

    async fn lookup_op(&self, op_hash: &DhtOpHash) -> EventResult<Db> {
        let sql = "
            SELECT when_stored
            FROM DhtOp
            WHERE hash = :op_hash
        ";

        let op_hash_clone = op_hash.clone();
        let authored_timestamp: Option<Timestamp> = self
            .authored
            .read_async(move |txn| {
                txn.query_row_and_then(
                    sql,
                    named_params! {
                        ":op_hash": op_hash_clone,
                    },
                    |row| Ok(row.get("when_stored")?),
                )
                .optional()
                .map_err(StateMutationError::from)
            })
            .await?;

        let op_hash_clone = op_hash.clone();
        let dht_timestamp: Option<Timestamp> = self
            .dht
            .read_async(move |txn| {
                txn.query_row_and_then(
                    sql,
                    named_params! {
                        ":op_hash": op_hash_clone,
                    },
                    |row| Ok(row.get("when_stored")?),
                )
                .optional()
                .map_err(StateMutationError::from)
            })
            .await?;

        match (authored_timestamp, dht_timestamp) {
            (Some(authored_timestamp), None) => Ok(Db::Authored),
            (None, Some(dht_timestamp)) => Ok(Db::Dht),
            (Some(authored_timestamp), Some(dht_timestamp)) => {
                if authored_timestamp < dht_timestamp {
                    Ok(Db::Authored)
                } else {
                    Ok(Db::Dht)
                }
            }
            (None, None) => Err(EventError::RequisiteEventNotFound(op_hash.clone())),
        }
    }

    pub async fn apply_event(&self, event: Event) -> EventResult<()> {
        let timestamp = event.timestamp;
        match event.data {
            EventData::Op(event) => match event {
                OpEvent::Authored { op } => {
                    dbg!(&op.to_hash());
                    let op = op.into_hashed();
                    self.authored
                        .write_async(move |txn| insert_op_when(txn, &op, timestamp))
                        .await?;
                }
                OpEvent::Fetched { op } => {
                    dbg!(&op.to_hash());
                    let op = op.into_hashed();
                    self.dht
                        .write_async(move |txn| insert_op_when(txn, &op, timestamp))
                        .await?;
                }
                OpEvent::SysValidated { op: op_hash } => {
                    let db = self.lookup_op(&op_hash).await?;
                    dbg!(&op_hash, db);
                    self.with_db(db, move |txn| {
                        set_when_sys_validated(txn, &op_hash, timestamp)
                    })
                    .await?;
                }
                OpEvent::AppValidated { op: op_hash } => {
                    let db = self.lookup_op(&op_hash).await?;
                    dbg!(&op_hash, db);
                    self.with_db(db, move |txn| {
                        set_when_app_validated(txn, &op_hash, timestamp)
                    })
                    .await?;
                }
                OpEvent::Integrated { op: op_hash } => {
                    let db = self.lookup_op(&op_hash).await?;
                    dbg!(&op_hash, db);
                    self.with_db(db, move |txn| set_when_integrated(txn, &op_hash, timestamp))
                        .await?;
                }
            },
        }
        Ok(())
    }

    pub async fn apply_events(&self, events: Vec<Event>) -> EventResult<()> {
        for event in events {
            self.apply_event(event).await?;
        }

        Ok(())
    }

    pub async fn get_events(&self) -> StateQueryResult<Vec<Event>> {
        let sql_common = "
            SELECT
            Action.blob as action_blob,
            Entry.blob as entry_blob,
            DhtOp.type as dht_type,
            DhtOp.when_stored,
            DhtOp.when_sys_validated,
            DhtOp.when_app_validated,
            DhtOp.when_integrated
            FROM Action
            JOIN
            DhtOp ON DhtOp.action_hash = Action.hash
            LEFT JOIN
            Entry ON Action.entry_hash = Entry.hash
            ORDER BY DhtOp.when_stored ASC
        ";
        let events_authored = self
            .authored
            .read_async(|txn| {
                txn.prepare_cached(sql_common)?
                    .query_and_then([], |row| {
                        let timestamp: Timestamp = row.get("when_stored")?;
                        let op = map_sql_dht_op(true, "dht_type", row)?;
                        let op_hash = op.to_hash();

                        // The existence of an op implies the Authored event
                        let mut events = vec![Event::new(timestamp, OpEvent::Authored { op })];

                        // The existence of a when_sys_validated timestamp
                        // implies the SysValidated event
                        if let Some(when_sys_validated) = row.get("when_sys_validated")? {
                            let ev = OpEvent::SysValidated {
                                op: op_hash.clone(),
                            };
                            events.push(Event::new(when_sys_validated, ev));
                        }

                        // The existence of a when_app_validated timestamp
                        // implies the AppValidated event
                        if let Some(when_app_validated) = row.get("when_app_validated")? {
                            let ev = OpEvent::AppValidated {
                                op: op_hash.clone(),
                            };
                            events.push(Event::new(when_app_validated, ev));
                        }

                        // The existence of a when_integrated timestamp
                        // implies the Integrated event
                        if let Some(when_integrated) = row.get("when_integrated")? {
                            let ev = OpEvent::Integrated {
                                op: op_hash.clone(),
                            };
                            events.push(Event::new(when_integrated, ev));
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
                        let timestamp: Timestamp = row.get("when_stored")?;
                        let op = map_sql_dht_op(true, "dht_type", row)?;
                        let op_hash = op.to_hash();

                        // The existence of an op implies the Fetched event
                        let mut events = vec![Event::new(timestamp, OpEvent::Fetched { op })];

                        // The existence of a when_sys_validated timestamp
                        // implies the SysValidated event
                        if let Some(when_sys_validated) = row.get("when_sys_validated")? {
                            let ev = OpEvent::SysValidated {
                                op: op_hash.clone(),
                            };
                            events.push(Event::new(when_sys_validated, ev));
                        }

                        // The existence of a when_app_validated timestamp
                        // implies the AppValidated event
                        if let Some(when_app_validated) = row.get("when_app_validated")? {
                            let ev = OpEvent::AppValidated {
                                op: op_hash.clone(),
                            };
                            events.push(Event::new(when_app_validated, ev));
                        }

                        // The existence of a when_integrated timestamp
                        // implies the Integrated event
                        if let Some(when_integrated) = row.get("when_integrated")? {
                            let ev = OpEvent::Integrated {
                                op: op_hash.clone(),
                            };
                            events.push(Event::new(when_integrated, ev));
                        }

                        StateQueryResult::Ok(events)
                    })?
                    .collect::<Result<Vec<Vec<_>>, _>>()
            })
            .await?;

        let mut events = events_authored
            .into_iter()
            .chain(events_dht.into_iter())
            .flatten()
            .collect::<Vec<_>>();

        // Ord is by timestamp, so this sorts the events in chronological order
        events.sort();

        Ok(events)
    }

    async fn with_db<F>(&self, db: Db, f: F) -> StateMutationResult<()>
    where
        F: Send + 'static + FnOnce(&mut Transaction) -> Result<(), StateMutationError>,
    {
        match db {
            Db::Authored => self.authored.write_async(f).await,
            Db::Dht => self.dht.write_async(f).await,
        }
    }
}

#[cfg(test)]
mod tests {

    use std::collections::BTreeSet;

    use super::*;
    use ::fixt::prelude::*;
    use arbitrary::Arbitrary;
    use maplit::btreeset;

    async fn db_roundtrip(
        store: &mut OpEventStore,
        events: impl Iterator<Item = &Event>,
    ) -> BTreeSet<Event> {
        store.apply_events(events.cloned().collect()).await.unwrap();
        store.get_events().await.unwrap().into_iter().collect()
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_event_creation() {
        use OpEvent::*;

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

        let events_1 = btreeset![
            Event::now(Authored { op: ops[0].clone() }),
            Event::now(Authored { op: ops[1].clone() }),
            Event::now(Authored { op: ops[2].clone() }),
            Event::now(SysValidated {
                op: ops[0].to_hash(),
            }),
            Event::now(AppValidated {
                op: ops[0].to_hash(),
            }),
            Event::now(Integrated {
                op: ops[0].to_hash(),
            }),
        ];
        let mut store_1 = OpEventStore::new_test(cell_id_1);
        let extracted_events_1 = db_roundtrip(&mut store_1, events_1.iter()).await;
        pretty_assertions::assert_eq!(events_1, extracted_events_1);

        // Setup store 2

        let events_2 = btreeset![
            Event::now(Fetched { op: ops[0].clone() }),
            Event::now(Fetched { op: ops[1].clone() }),
            Event::now(Fetched { op: ops[2].clone() }),
        ];
        let mut store_2 = OpEventStore::new_test(cell_id_2);
        let extracted_events_2 = db_roundtrip(&mut store_2, events_2.iter()).await;
        pretty_assertions::assert_eq!(events_2, extracted_events_2);
    }
}
