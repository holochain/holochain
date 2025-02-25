use bytes::Bytes;
use futures::future::BoxFuture;
use holo_hash::{DhtOpHash, DnaHash};
use holochain_serialized_bytes::prelude::decode;
use holochain_sqlite::db::{DbKindDht, DbWrite};
use holochain_sqlite::rusqlite::types::Value;
use holochain_sqlite::sql::sql_dht::{
    OPS_BY_ID, OP_HASHES_IN_TIME_SLICE, OP_HASHES_SINCE_TIME_BATCH,
};
use holochain_state::prelude::{named_params, StateMutationResult};
use holochain_types::dht_op::DhtOpHashed;
use holochain_types::prelude::DhtOp;
use kitsune2_api::*;
use std::collections::HashSet;
use std::fmt::{Debug, Formatter};
use std::rc::Rc;
use std::sync::Arc;

/// Holochain implementation of the Kitsune2 [OpStoreFactory].
pub struct HolochainOpStoreFactory {
    /// The database connection getter.
    pub getter: crate::GetDbOpStore,
    /// The event handler.
    pub handler: crate::types::event::DynHcP2pHandler,
}

impl std::fmt::Debug for HolochainOpStoreFactory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HolochainOpStoreFactory").finish()
    }
}

impl kitsune2_api::OpStoreFactory for HolochainOpStoreFactory {
    fn default_config(&self, _config: &mut kitsune2_api::Config) -> kitsune2_api::K2Result<()> {
        Ok(())
    }

    fn validate_config(&self, _config: &kitsune2_api::Config) -> kitsune2_api::K2Result<()> {
        Ok(())
    }

    fn create(
        &self,
        _builder: Arc<kitsune2_api::Builder>,
        space: kitsune2_api::SpaceId,
    ) -> BoxFut<'static, kitsune2_api::K2Result<kitsune2_api::DynOpStore>> {
        let getter = self.getter.clone();
        let handler = self.handler.clone();
        Box::pin(async move {
            let dna_hash = DnaHash::from_k2_space(&space);
            let db = getter(dna_hash.clone()).await.map_err(|err| {
                kitsune2_api::K2Error::other_src("failed to get op_store db", err)
            })?;
            let op_store: kitsune2_api::DynOpStore =
                Arc::new(HolochainOpStore::new(db, dna_hash, handler));

            Ok(op_store)
        })
    }
}

/// Holochain implementation of the Kitsune2 [OpStore].
pub struct HolochainOpStore {
    db: DbWrite<DbKindDht>,
    dna_hash: DnaHash,
    sender: crate::event::DynHcP2pHandler,
}

impl Debug for HolochainOpStore {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HolochainOpStore")
            .field("db", &self.db)
            .finish()
    }
}

impl HolochainOpStore {
    /// Create a new [HolochainOpStore].
    pub fn new(
        db: DbWrite<DbKindDht>,
        dna_hash: DnaHash,
        sender: crate::event::DynHcP2pHandler,
    ) -> HolochainOpStore {
        Self {
            db,
            dna_hash,
            sender,
        }
    }
}

impl OpStore for HolochainOpStore {
    fn process_incoming_ops(&self, op_list: Vec<Bytes>) -> BoxFut<'_, K2Result<Vec<OpId>>> {
        Box::pin(async move {
            let dht_ops = op_list
                .into_iter()
                // Filter to make casting the size below safe
                .filter(|op| op.len() <= u32::MAX as usize)
                .map(|op| {
                    Ok((
                        op.len() as u32,
                        decode::<_, DhtOp>(op.as_ref())
                            .map_err(|e| K2Error::other_src("Could not decode op", e))?,
                    ))
                })
                .collect::<K2Result<Vec<(u32, DhtOp)>>>()?;

            let ids = dht_ops
                .iter()
                .map(|(_, op)| {
                    let op_hashed = DhtOpHashed::from_content_sync(op.clone());
                    op_hashed.hash.to_k2_op()
                })
                .collect();

            self.sender
                .publish(
                    self.dna_hash.clone(),
                    false,
                    false,
                    dht_ops.into_iter().map(|(_, op)| op).collect(),
                )
                .await
                .map_err(|e| K2Error::other_src("Failed to publish incoming ops", e))?;

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

                        let op_id = hash.to_k2_op();
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
                                let hash = DhtOpHash::from_k2_op(id);
                                Value::from(hash.into_inner())
                            })
                            .collect::<Vec<_>>(),
                    )])?;

                    let mut out = Vec::new();
                    while let Some(row) = rows.next()? {
                        let hash: DhtOpHash = row.get(0)?;
                        let dht_op = holochain_state::query::map_sql_dht_op(false, "type", row)?;

                        out.push(MetaOp {
                            op_id: hash.to_k2_op(),
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
        let db = self.db.clone();

        let (arc_start, arc_end) = match arc {
            DhtArc::Empty => {
                return Box::pin(async move { Ok((vec![], 0, start)) });
            }
            DhtArc::Arc(start, end) => (start, end),
        };

        Box::pin(async move {
            let out = db
                .read_async(
                    move |txn| -> StateMutationResult<(Vec<OpId>, u32, Timestamp)> {
                        let mut used_bytes = 0;
                        let mut latest_timestamp = start;
                        let mut out = HashSet::new();

                        'outer: loop {
                            let mut stmt = txn.prepare(OP_HASHES_SINCE_TIME_BATCH)?;
                            let mut rows = match stmt.query(named_params! {
                                ":storage_start_loc": arc_start,
                                ":storage_end_loc": arc_end,
                                ":timestamp_min": latest_timestamp.as_micros(),
                                // Fetch ops in batches of 500. This lets us observe the `limit_bytes`
                                // without going to the database too many times.
                                // Because the timestamp being queried is the integration timestamp,
                                // it shouldn't be possible for >500 ops authored at the same time
                                // to prevent this loop from proceeding.
                                ":limit": 500,
                            }) {
                                Ok(rows) => rows,
                                Err(e) => return Err(e.into()),
                            };

                            let ops_size = out.len();

                            while let Some(row) = rows.next()? {
                                let hash: DhtOpHash = row.get(0)?;
                                let timestamp = Timestamp::from_micros(row.get::<_, i64>(1)?);
                                let serialized_size: u32 = row.get(2)?;

                                if used_bytes + serialized_size > limit_bytes {
                                    break 'outer;
                                }

                                let op_id = hash.to_k2_op();
                                if out.insert(op_id) {
                                    latest_timestamp = timestamp;
                                    used_bytes += serialized_size;
                                }
                            }

                            // If we didn't discover any new ops, break
                            if out.len() == ops_size {
                                break;
                            }
                        }

                        Ok((out.into_iter().collect(), used_bytes, latest_timestamp))
                    },
                )
                .await
                .map_err(|e| K2Error::other_src("Failed to retrieve op ids bounded", e))?;

            Ok(out)
        })
    }

    fn store_slice_hash(
        &self,
        arc: DhtArc,
        slice_index: u64,
        slice_hash: Bytes,
    ) -> BoxFuture<'_, K2Result<()>> {
        let db = self.db.clone();

        let (arc_start, arc_end) = match arc {
            DhtArc::Empty => {
                return Box::pin(async move { Ok(()) });
            }
            DhtArc::Arc(start, end) => (start, end),
        };

        Box::pin(async move {
            db.write_async(move |txn| -> StateMutationResult<()> {
                let mut stmt = txn.prepare(
                    r#"INSERT INTO SliceHash
                (arc_start, arc_end, slice_index, hash)
                VALUES (:arc_start, :arc_end, :slice_index, :hash)"#,
                )?;

                stmt.execute(named_params! {
                    ":arc_start": arc_start,
                    ":arc_end": arc_end,
                    ":slice_index": slice_index,
                    ":hash": slice_hash.to_vec(),
                })?;

                Ok(())
            })
            .await
            .map_err(|e| K2Error::other_src("Failed to store slice hash", e))?;

            Ok(())
        })
    }

    fn slice_hash_count(&self, arc: DhtArc) -> BoxFuture<'_, K2Result<u64>> {
        let db = self.db.clone();

        let (arc_start, arc_end) = match arc {
            DhtArc::Empty => {
                return Box::pin(async move { Ok(0) });
            }
            DhtArc::Arc(start, end) => (start, end),
        };

        Box::pin(async move {
            let out = db
                .read_async(move |txn| -> StateMutationResult<u64> {
                    let mut stmt = txn.prepare(
                        r#"SELECT MAX(slice_index) FROM SliceHash
                    WHERE arc_start = :arc_start AND arc_end = :arc_end"#,
                    )?;

                    let count = match stmt.query_row(
                        named_params! {
                            ":arc_start": arc_start,
                            ":arc_end": arc_end,
                        },
                        |r| r.get(0),
                    ) {
                        Ok(count) => count,
                        Err(holochain_sqlite::rusqlite::Error::QueryReturnedNoRows) => 0,
                        Err(e) => return Err(e.into()),
                    };

                    Ok(count)
                })
                .await
                .map_err(|e| K2Error::other_src("Failed to count slice hashes", e))?;

            Ok(out)
        })
    }

    fn retrieve_slice_hash(
        &self,
        arc: DhtArc,
        slice_index: u64,
    ) -> BoxFuture<'_, K2Result<Option<Bytes>>> {
        let db = self.db.clone();

        let (arc_start, arc_end) = match arc {
            DhtArc::Empty => {
                return Box::pin(async move { Ok(None) });
            }
            DhtArc::Arc(start, end) => (start, end),
        };

        Box::pin(async move {
            let out = db
                .read_async(move |txn| -> StateMutationResult<Option<Bytes>> {
                    let mut stmt = txn.prepare(r#"SELECT hash FROM SliceHash
                    WHERE arc_start = :arc_start AND arc_end = :arc_end AND slice_index = :slice_index"#)?;

                    let hash = match stmt.query_row(named_params! {
                        ":arc_start": arc_start,
                        ":arc_end": arc_end,
                        ":slice_index": slice_index,
                    }, |r| r.get::<_, Vec<u8>>(0)) {
                        Ok(hash) => Some(Bytes::from(hash)),
                        Err(holochain_sqlite::rusqlite::Error::QueryReturnedNoRows) => None,
                        Err(e) => return Err(e.into()),
                    };

                    Ok(hash)
                })
                .await
                .map_err(|e| K2Error::other_src("Failed to retrieve slice hash", e))?;

            Ok(out)
        })
    }

    fn retrieve_slice_hashes(&self, arc: DhtArc) -> BoxFuture<'_, K2Result<Vec<(u64, Bytes)>>> {
        let db = self.db.clone();

        let (arc_start, arc_end) = match arc {
            DhtArc::Empty => {
                return Box::pin(async move { Ok(vec![]) });
            }
            DhtArc::Arc(start, end) => (start, end),
        };

        Box::pin(async move {
            let out = db
                .read_async(move |txn| -> StateMutationResult<Vec<(u64, Bytes)>> {
                    let mut stmt = txn.prepare(
                        r#"SELECT slice_index, hash FROM SliceHash
                    WHERE arc_start = :arc_start AND arc_end = :arc_end"#,
                    )?;

                    let hash = stmt
                        .query_map(
                            named_params! {
                                ":arc_start": arc_start,
                                ":arc_end": arc_end,
                            },
                            |r| Ok((r.get::<_, u64>(0)?, Bytes::from(r.get::<_, Vec<u8>>(1)?))),
                        )?
                        .collect::<holochain_sqlite::rusqlite::Result<Vec<_>>>()?;

                    Ok(hash)
                })
                .await
                .map_err(|e| K2Error::other_src("Failed to retrieve slice hashes", e))?;

            Ok(out)
        })
    }
}
