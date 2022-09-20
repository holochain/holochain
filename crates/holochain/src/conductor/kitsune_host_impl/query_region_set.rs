use std::sync::{
    atomic::{AtomicI64, Ordering},
    Arc,
};

use holochain_p2p::{dht::prelude::*, dht_arc::DhtArcSet};
use holochain_sqlite::prelude::*;
use rusqlite::named_params;

use crate::conductor::error::ConductorResult;

static LAST_LOG_MS: AtomicI64 = AtomicI64::new(0);
const LOG_RATE_MS: i64 = 1000;

/// The network module needs info about various groupings ("regions") of ops
pub async fn query_region_set(
    db: DbWrite<DbKindDht>,
    topology: Topology,
    strat: &ArqStrat,
    dht_arc_set: Arc<DhtArcSet>,
) -> ConductorResult<RegionSetLtcs> {
    let (arq_set, rounded) = ArqBoundsSet::from_dht_arc_set_rounded(&topology, strat, &dht_arc_set);
    if rounded {
        // If an arq was rounded, emit a warning, but throttle it to once every LOG_RATE_MS
        // so we don't get slammed.
        let _ = LAST_LOG_MS.fetch_update(Ordering::Relaxed, Ordering::Relaxed, |t| {
            let now = Timestamp::now();
            let it_is_time = now
                .checked_difference_signed(&Timestamp::from_micros(t * 1000))
                .map(|d| d > chrono::Duration::milliseconds(LOG_RATE_MS))
                .unwrap_or(false);
            if it_is_time {
                tracing::warn!(
                    "A continuous arc set could not be properly quantized.
                Original:  {:?}
                Quantized: {:?}",
                    dht_arc_set,
                    arq_set
                );
                Some(now.as_millis())
            } else {
                None
            }
        });
    }

    let times = TelescopingTimes::historical(&topology);
    let coords = RegionCoordSetLtcs::new(times, arq_set);

    let region_set = db
        .async_reader(move |txn| {
            let sql = holochain_sqlite::sql::sql_cell::FETCH_OP_REGION;
            let mut stmt = txn.prepare_cached(sql).map_err(DatabaseError::from)?;
            DatabaseResult::Ok(
                coords.into_region_set(|(_, coords)| {
                    query_region_data(&mut stmt, &topology, coords)
                })?,
            )
        })
        .await?;

    Ok(region_set)
}

pub(super) fn query_region_data(
    stmt: &mut rusqlite::CachedStatement,
    topology: &Topology,
    coords: RegionCoords,
) -> Result<RegionData, DatabaseError> {
    let bounds = coords.to_bounds(topology);
    let (x0, x1) = bounds.x;
    let (t0, t1) = bounds.t;
    stmt.query_row(
        named_params! {
            ":storage_start_loc": x0,
            ":storage_end_loc": x1,
            ":timestamp_min": t0,
            ":timestamp_max": t1,
        },
        |row| {
            let size: f64 = row.get("total_size")?;
            Ok(RegionData {
                hash: RegionHash::from_vec(row.get("xor_hash")?)
                    .expect("region hash must be 32 bytes"),
                size: size.min(u32::MAX as f64) as u32,
                count: row.get("count")?,
            })
        },
    )
    .map_err(DatabaseError::from)
}
