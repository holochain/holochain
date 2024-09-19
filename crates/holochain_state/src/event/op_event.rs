//! This module defines the two-way mapping between OpEvents and the DhtOp databases.
//!
//! OpEvents are concerned with the lifecycle of DhtOps, from their creation, to
//! their validation and integration.
//!
//! The database state pertaining to DhtOps can be streamed out as a list of `OpEvent`s,
//! and that same state can be recreated by applying a list of `OpEvent`s.

use crate::{prelude::*, query::map_sql_dht_op};

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

    /// The node has received a validation receipt from another
    /// agent for op it authored
    ReceivedValidationReceipt { receipt: SignedValidationReceipt },
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

    pub async fn apply_event(&self, event: Event) -> EventResult<()> {
        let timestamp = event.timestamp;
        match event.data {
            EventData::Op(event) => match event {
                OpEvent::Authored { op } => {
                    let op = op.into_hashed();
                    self.authored
                        .write_async(move |txn| insert_op_when(txn, &op, timestamp))
                        .await?;
                }
                OpEvent::Fetched { op } => {
                    let op = op.into_hashed();
                    self.dht
                        .write_async(move |txn| insert_op_when(txn, &op, timestamp))
                        .await?;
                }
                OpEvent::SysValidated { op: op_hash } => {
                    self.dht
                        .write_async(move |txn| set_when_sys_validated(txn, &op_hash, timestamp))
                        .await?;
                }
                OpEvent::AppValidated { op: op_hash } => {
                    self.dht
                        .write_async(move |txn| set_when_app_validated(txn, &op_hash, timestamp))
                        .await?;
                }
                OpEvent::Integrated { op: op_hash } => {
                    self.dht
                        .write_async(move |txn| set_when_integrated(txn, &op_hash, timestamp))
                        .await?;
                }
                OpEvent::ReceivedValidationReceipt { receipt } => {
                    self.authored
                        .write_async(move |txn| {
                            insert_validation_receipt_when(txn, receipt, timestamp)
                        })
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
        let sql_ops = "
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

        let sql_receipts = "
            SELECT
            ValidationReceipt.blob as receipt_blob,
            ValidationReceipt.when_received
            FROM ValidationReceipt
            ORDER BY ValidationReceipt.when_received ASC
        ";

        let events_authored = self
            .authored
            .read_async(|txn| {
                let mut events = Vec::new();

                txn.prepare_cached(sql_ops)?
                    .query_and_then([], |row| {
                        let timestamp: Timestamp = row.get("when_stored")?;
                        let op = map_sql_dht_op(true, "dht_type", row)?;

                        // The existence of an op implies the Authored event
                        events.push(Event::new(timestamp, OpEvent::Authored { op }));

                        // More events to come:
                        // - [ ] Published

                        StateQueryResult::Ok(())
                    })?
                    .collect::<Result<Vec<()>, _>>()?;

                txn.prepare_cached(sql_receipts)?
                    .query_and_then([], |row| {
                        let timestamp: Timestamp = row.get("when_received")?;
                        let receipt =
                            from_blob::<SignedValidationReceipt>(row.get("receipt_blob")?)?;

                        // The existence of a receipt implies the ReceivedValidationReceipt event
                        events.push(Event::new(
                            timestamp,
                            OpEvent::ReceivedValidationReceipt { receipt },
                        ));

                        StateQueryResult::Ok(())
                    })?
                    .collect::<Result<Vec<()>, _>>()?;

                StateQueryResult::Ok(events)
            })
            .await?;

        let events_dht = self
            .dht
            .read_async(|txn| {
                let mut events = Vec::new();

                txn.prepare_cached(sql_ops)?
                    .query_and_then([], |row| {
                        let timestamp: Timestamp = row.get("when_stored")?;
                        let op = map_sql_dht_op(true, "dht_type", row)?;
                        let op_hash = op.to_hash();

                        // The existence of an op implies the Fetched event
                        events.push(Event::new(timestamp, OpEvent::Fetched { op }));

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

                        StateQueryResult::Ok(())
                    })?
                    .collect::<Result<Vec<()>, _>>()?;
                StateQueryResult::Ok(events)
            })
            .await?;

        let mut events = events_authored
            .into_iter()
            .chain(events_dht.into_iter())
            .collect::<Vec<_>>();

        // Ord is by timestamp, so this sorts the events in chronological order
        events.sort();

        Ok(events)
    }
}

#[cfg(test)]
mod tests {

    use std::collections::BTreeSet;

    use super::*;
    use ::fixt::prelude::*;
    use arbitrary::Arbitrary;
    use holochain_keystore::test_keystore;
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

        let keystore = test_keystore();

        let agent_1 = keystore.new_sign_keypair_random().await.unwrap();
        let agent_2 = keystore.new_sign_keypair_random().await.unwrap();
        let dna_hash = fixt!(DnaHash);
        let cell_id_1 = CellId::new(dna_hash.clone(), agent_1);
        let cell_id_2 = CellId::new(dna_hash.clone(), agent_2);

        let mut receipt = ValidationReceipt::arbitrary(&mut u).unwrap();
        receipt.validators = vec![cell_id_1.agent_pubkey().clone()];
        receipt.dht_op_hash = ops[0].to_hash();
        let receipt = receipt.sign(&keystore).await.unwrap().unwrap();

        // Setup store 1

        let events_1 = btreeset![
            Event::now(Authored { op: ops[0].clone() }),
            Event::now(Authored { op: ops[1].clone() }),
            Event::now(Authored { op: ops[2].clone() }),
            Event::now(ReceivedValidationReceipt { receipt }),
        ];
        let mut store_1 = OpEventStore::new_test(cell_id_1);
        let extracted_events_1 = db_roundtrip(&mut store_1, events_1.iter()).await;
        pretty_assertions::assert_eq!(events_1, extracted_events_1);

        // Setup store 2

        let events_2 = btreeset![
            Event::now(Fetched { op: ops[0].clone() }),
            Event::now(Fetched { op: ops[1].clone() }),
            Event::now(Fetched { op: ops[2].clone() }),
            // op 0 is integrated
            Event::now(SysValidated {
                op: ops[0].to_hash(),
            }),
            Event::now(AppValidated {
                op: ops[0].to_hash(),
            }),
            Event::now(Integrated {
                op: ops[0].to_hash(),
            }),
            // op 1 is merely app validated
            Event::now(SysValidated {
                op: ops[1].to_hash(),
            }),
            Event::now(AppValidated {
                op: ops[1].to_hash(),
            }),
            // op 2 is merely sys validated
            Event::now(SysValidated {
                op: ops[2].to_hash(),
            }),
        ];
        let mut store_2 = OpEventStore::new_test(cell_id_2);
        let extracted_events_2 = db_roundtrip(&mut store_2, events_2.iter()).await;
        pretty_assertions::assert_eq!(events_2, extracted_events_2);
    }
}
