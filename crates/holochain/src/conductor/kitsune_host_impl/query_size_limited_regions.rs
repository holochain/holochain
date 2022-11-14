use holochain_p2p::dht::prelude::*;
use holochain_sqlite::prelude::*;
use holochain_types::prelude::first_ref;

use crate::conductor::error::ConductorResult;

use super::query_region_set::query_region_data;

/// Given a set of Regions, return an equivalent set of Regions (which covers the same
/// area of the DHT) such that no region is larger than `size_limit`.
/// Regions larger than size_limit will be quadrisected, and the size of each subregion
/// will be fetched from the database. The quadrisecting is recursive until either all
/// regions are either small enough, or cannot be further subdivided.
pub async fn query_size_limited_regions(
    db: DbWrite<DbKindDht>,
    topology: Topology,
    regions: Vec<Region>,
    size_limit: u32,
) -> ConductorResult<Vec<Region>> {
    Ok(db
        .async_reader(move |txn| {
            let sql = holochain_sqlite::sql::sql_cell::FETCH_OP_REGION;
            let mut stmt = txn.prepare_cached(sql).map_err(DatabaseError::from)?;

            // The regions whose size has not been checked. A `true` boolean specifies that
            // the region could not be subdivided, and should be returned as-is.
            let mut unchecked: Vec<(Region, bool)> =
                regions.into_iter().map(|r| (r, false)).collect();
            let mut checked: Vec<Region> = vec![];

            while !unchecked.is_empty() {
                // partition the set into regions which need to be split, and those which don't
                let (smalls, bigs): (Vec<_>, Vec<_>) = unchecked
                    .iter()
                    // If the region is locked, the size is considered 0, so a locked region
                    // will not be a candidate for quandrisection
                    .partition(|(r, quantum)| *quantum || r.data.size() <= size_limit);
                // add the unsplittables to the final set to be returned
                checked.extend(smalls.into_iter().map(first_ref).cloned());

                // split up the splittables and check the new sizes, using this list
                // as the starting point of the next iteration
                unchecked = bigs
                    .into_iter()
                    .flat_map(|(r, _)| {
                        r.coords
                            .quadrisect()
                            .map(|rs| rs.into_iter().map(|r| (r, false)).collect())
                            .unwrap_or_else(|| vec![(r.coords, true)])
                    })
                    .map(|(c, q)| {
                        let data = query_region_data(&mut stmt, &topology, c)?;
                        DatabaseResult::Ok((Region::new(c, RegionCell::Data(data)), q))
                    })
                    .collect::<Result<Vec<(Region, bool)>, _>>()?;
            }
            DatabaseResult::Ok(checked)
        })
        .await?)
}
