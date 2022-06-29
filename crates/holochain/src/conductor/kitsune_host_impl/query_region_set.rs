use std::sync::Arc;

use holochain_p2p::{dht::prelude::*, dht_arc::DhtArcSet};
use holochain_sqlite::prelude::*;
use kitsune_p2p_types::config::KitsuneP2pTuningParams;
use rusqlite::named_params;

use crate::conductor::error::ConductorResult;

/// The network module needs info about various groupings ("regions") of ops
pub async fn query_region_set(
    db: DbWrite<DbKindAuthored>,
    topology: Topology,
    strat: &ArqStrat,
    dht_arc_set: Arc<DhtArcSet>,
    tuning_params: &KitsuneP2pTuningParams,
) -> ConductorResult<RegionSetLtcs> {
    let arq_set = ArqBoundsSet::from_dht_arc_set(&topology, &strat, &dht_arc_set)
        .expect("arc is not quantizable (FIXME: only use quantized arcs)");
    let recent_threshold =
        std::time::Duration::from_secs(tuning_params.danger_gossip_recent_threshold_secs);
    let times = TelescopingTimes::historical(&topology, recent_threshold);
    let coords = RegionCoordSetLtcs::new(times, arq_set);

    let region_set = db
        .async_reader(move |txn| {
            let sql = holochain_sqlite::sql::sql_cell::FETCH_OP_REGION;
            let mut stmt = txn.prepare_cached(sql).map_err(DatabaseError::from)?;
            DatabaseResult::Ok(
                coords.into_region_set(|(_, coords)| {
                    query_region_coords(&mut stmt, &topology, coords)
                })?,
            )
        })
        .await?;

    Ok(region_set)
}

fn query_region_coords(
    stmt: &mut rusqlite::CachedStatement,
    topology: &Topology,
    coords: RegionCoords,
) -> Result<RegionData, DatabaseError> {
    let bounds = coords.to_bounds(&topology);
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
