use bytes::Bytes;
use futures::future::BoxFuture;
use holo_hash::DhtOpHash;
use holochain_serialized_bytes::prelude::decode;
use holochain_sqlite::db::{DbKindDht, DbWrite};
use holochain_sqlite::rusqlite::types::Value;
use holochain_sqlite::sql::sql_dht::{OPS_BY_ID, OP_HASHES_IN_TIME_SLICE};
use holochain_state::prelude::{named_params, StateMutationResult};
use holochain_types::dht_op::DhtOpHashed;
use holochain_types::prelude::DhtOp;
use kitsune2_api::*;
use std::rc::Rc;

/// Holochain implementation of the Kitsune2 [OpStore].
#[derive(Debug)]
pub struct HolochainOpStore {
    db: DbWrite<DbKindDht>,
}

impl HolochainOpStore {
    /// Create a new [HolochainOpStore].
    pub fn new(db: DbWrite<DbKindDht>) -> HolochainOpStore {
        Self { db }
    }
}

impl OpStore for HolochainOpStore {
    fn process_incoming_ops(&self, op_list: Vec<Bytes>) -> BoxFut<'_, K2Result<Vec<OpId>>> {
        let db = self.db.clone();
        Box::pin(async move {
            let dht_ops = op_list
                .into_iter()
                // Filter to make casting the size below safe
                .filter(|op| op.len() <= u32::MAX as usize)
                .map(|op| {
                    Ok((
                        op.len() as u32,
                        decode::<_, DhtOp>(op.as_ref())
                            .map(DhtOpHashed::from_content_sync)
                            .map_err(|e| K2Error::other_src("Could not decode op", e))?,
                    ))
                })
                .collect::<K2Result<Vec<(u32, DhtOpHashed)>>>()?;

            let ids = dht_ops
                .iter()
                .map(|(_, op)| OpId::from(Bytes::copy_from_slice(&op.hash.get_raw_36())))
                .collect();

            db.write_async(move |txn| -> StateMutationResult<()> {
                for (size, op) in &dht_ops {
                    holochain_state::prelude::insert_op_dht(txn, op, *size, None)?;
                }

                Ok(())
            })
            .await
            .map_err(|e| K2Error::other_src("Failed to insert op", e))?;

            Ok(ids)
        })
    }

    fn retrieve_op_hashes_in_time_slice(
        &self,
        arc: DhtArc,
        start: Timestamp,
        end: Timestamp,
    ) -> BoxFuture<'_, K2Result<(Vec<OpId>, u32)>> {
        let db = self.db.clone();

        let (arc_start, arc_end) = match arc {
            DhtArc::Empty => {
                return Box::pin(async move { Ok((vec![], 0)) });
            }
            DhtArc::Arc(start, end) => (start, end),
        };

        Box::pin(async move {
            let out = db
                .read_async(move |txn| -> StateMutationResult<(Vec<OpId>, u32)> {
                    let mut stmt = txn.prepare(OP_HASHES_IN_TIME_SLICE)?;

                    let mut rows = stmt.query(named_params! {
                        ":storage_start_loc": arc_start,
                        ":storage_end_loc": arc_end,
                        ":timestamp_min": start.as_micros(),
                        ":timestamp_max": end.as_micros(),
                    })?;

                    let mut out = Vec::new();
                    let mut out_size = 0;
                    while let Some(row) = rows.next()? {
                        let hash: DhtOpHash = row.get(0)?;
                        let serialized_size: u32 = row.get(1)?;

                        let op_id = OpId::from(Bytes::copy_from_slice(&hash.get_raw_36()));
                        out.push(op_id);
                        out_size += serialized_size;
                    }

                    Ok((out, out_size))
                })
                .await
                .map_err(|e| K2Error::other_src("Failed to retrieve op hashes in time slice", e))?;

            Ok(out)
        })
    }

    /// Retrieve a list of ops by their op ids.
    ///
    /// This should be used to get op data for ops.
    fn retrieve_ops(&self, op_ids: Vec<OpId>) -> BoxFuture<'_, K2Result<Vec<MetaOp>>> {
        let db = self.db.clone();

        Box::pin(async move {
            let out = db
                .read_async(move |txn| -> StateMutationResult<Vec<MetaOp>> {
                    let mut stmt = txn.prepare(OPS_BY_ID)?;

                    let mut rows = stmt.query([Rc::new(
                        op_ids
                            .iter()
                            .map(|id| {
                                // Hashes in the database are the full 39 bytes so we need to
                                // do a little dance to get the type added to the 36 byte id
                                let hash = DhtOpHash::from_raw_36_and_type(
                                    id.as_ref().to_vec(),
                                    holo_hash::hash_type::DhtOp,
                                );
                                Value::from(hash.into_inner())
                            })
                            .collect::<Vec<_>>(),
                    )])?;

                    let mut out = Vec::new();
                    while let Some(row) = rows.next()? {
                        let hash: DhtOpHash = row.get(0)?;
                        let dht_op = holochain_state::query::map_sql_dht_op(false, "type", row)?;

                        out.push(MetaOp {
                            op_id: OpId::from(Bytes::copy_from_slice(&hash.get_raw_36())),
                            op_data: holochain_serialized_bytes::prelude::encode(&dht_op)?.into(),
                        });
                    }

                    Ok(out)
                })
                .await
                .map_err(|e| K2Error::other_src("Failed to retrieve ops", e))?;

            Ok(out)
        })
    }

    fn retrieve_op_ids_bounded(
        &self,
        arc: DhtArc,
        start: Timestamp,
        limit_bytes: u32,
    ) -> BoxFuture<'_, K2Result<(Vec<OpId>, u32, Timestamp)>> {
        todo!()
    }

    fn store_slice_hash(
        &self,
        arc: DhtArc,
        slice_index: u64,
        slice_hash: Bytes,
    ) -> BoxFuture<'_, K2Result<()>> {
        todo!()
    }

    fn slice_hash_count(&self, arc: DhtArc) -> BoxFuture<'_, K2Result<u64>> {
        todo!()
    }

    fn retrieve_slice_hash(
        &self,
        arc: DhtArc,
        slice_index: u64,
    ) -> BoxFuture<'_, K2Result<Option<Bytes>>> {
        todo!()
    }

    fn retrieve_slice_hashes(&self, arc: DhtArc) -> BoxFuture<'_, K2Result<Vec<(u64, Bytes)>>> {
        todo!()
    }
}
