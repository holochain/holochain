use std::sync::Arc;

use holochain_p2p::{dht::prelude::*, dht_arc::DhtArcSet};
use holochain_sqlite::prelude::*;
use rusqlite::named_params;

use crate::conductor::error::ConductorResult;

/// The network module needs info about various groupings ("regions") of ops
pub async fn query_region_set(
    db: DbWrite<DbKindAuthored>,
    topology: Topology,
    // author: AgentPubKey,
    dht_arc_set: Arc<DhtArcSet>,
) -> ConductorResult<RegionSetXtcs> {
    let sql = holochain_sqlite::sql::sql_cell::FETCH_OP_REGION;
    let max_chunks = ArqStrat::default().max_chunks();
    let arq_set = ArqBoundsSet::new(
        dht_arc_set
            .intervals()
            .into_iter()
            .map(|i| {
                let len = i.length();
                let (pow, _) = power_and_count_from_length(len, max_chunks);
                ArqBounds::from_interval_rounded(pow, i)
            })
            .collect(),
    );
    // TODO: This should be behind the current moment by however much Recent gossip covers.
    let current = Timestamp::now();
    let times = TelescopingTimes::new(TimeQuantum::from_timestamp(&topology, current));
    let coords = RegionCoordSetXtcs::new(times, arq_set);
    let coords_clone = coords.clone();
    let data = db
        .async_reader(move |txn| {
            let mut stmt = txn.prepare_cached(sql).map_err(DatabaseError::from)?;
            coords_clone
                .region_coords_nested()
                .map(|column| {
                    column
                        .map(|(_, coords)| {
                            let bounds = coords.to_bounds(&topology);
                            let (x0, x1) = bounds.x;
                            let (t0, t1) = bounds.t;
                            stmt.query_row(
                                named_params! {
                                    ":storage_start_loc": x0,
                                    ":storage_end_loc": x1,
                                    ":timestamp_min": t0,
                                    ":timestamp_max": t1,
                                    // ":author": &author, // TODO: unneeded for authored table?
                                },
                                |row| {
                                    Ok(RegionData {
                                        hash: RegionHash::from_vec(row.get("hash")?)
                                            .expect("region hash must be 32 bytes"),
                                        size: row.get("size")?,
                                        count: row.get("count")?,
                                    })
                                },
                            )
                        })
                        .collect::<Result<Vec<RegionData>, rusqlite::Error>>()
                        .map_err(DatabaseError::from)
                })
                .collect::<Result<Vec<Vec<RegionData>>, DatabaseError>>()
        })
        .await?;
    Ok(RegionSetXtcs::from_data(coords, data))
}
