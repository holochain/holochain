use holo_hash::DhtOpHash;
use holochain_p2p::{dht::prelude::*, DhtOpHashExt};
use holochain_sqlite::prelude::*;
use kitsune_p2p::dependencies::kitsune_p2p_fetch::OpHashSized;
use rusqlite::named_params;

use crate::conductor::error::ConductorResult;

pub(super) async fn query_region_op_hashes(
    db: DbWrite<DbKindDht>,
    bounds: RegionBounds,
) -> ConductorResult<Vec<OpHashSized>> {
    Ok(db
        .async_reader(move |txn| {
            let sql = holochain_sqlite::sql::sql_cell::FETCH_REGION_OP_HASHES;
            let mut stmt = txn.prepare_cached(sql).map_err(DatabaseError::from)?;
            let (x0, x1) = bounds.x;
            let (t0, t1) = bounds.t;
            let hashes = stmt
                .query_map(
                    named_params! {
                        ":storage_start_loc": x0,
                        ":storage_end_loc": x1,
                        ":timestamp_min": t0,
                        ":timestamp_max": t1,
                    },
                    |row| {
                        let hash: DhtOpHash = row.get("hash")?;
                        let action_size: usize = row.get("action_size")?;
                        // will be NULL if the op has no associated entry
                        let entry_size: Option<usize> = row.get("entry_size")?;
                        let op_size = (action_size + entry_size.unwrap_or(0)).into();
                        Ok(OpHashSized::new(hash.to_kitsune(), Some(op_size)))
                    },
                )?
                .collect::<Result<Vec<_>, _>>()
                .map_err(DatabaseError::from);
            hashes
        })
        .await?)
}
