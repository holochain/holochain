use std::sync::Arc;

use holochain_p2p::{dht::prelude::*, dht_arc::DhtArcSet};
use holochain_sqlite::prelude::*;
use rusqlite::named_params;

use crate::conductor::error::ConductorResult;

/// The network module needs info about various groupings ("regions") of ops
pub async fn query_region_set<O: AccessOpStore>(
    db: O,
    coords: RegionCoordSetXtcs,
) -> ConductorResult<RegionSetXtcs> {
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

    let data = db.fetch_region_set(coords.clone()).await?;
    Ok(RegionSetXtcs::from_data(coords, data))
}
