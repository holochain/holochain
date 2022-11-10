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
        let it_is_time = LAST_LOG_MS
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |t| {
                let now = Timestamp::now();
                // If the difference is greater than the logging interval,
                // produce a Some so that the atomic val gets updated, which
                // will trigger a log after the update.
                now.checked_difference_signed(&Timestamp::from_micros(t * 1000))
                    .map(|d| d > chrono::Duration::milliseconds(LOG_RATE_MS))
                    .unwrap_or(false)
                    .then(|| now.as_millis())
            })
            .is_ok();
        if it_is_time {
            tracing::warn!(
                "A continuous arc set could not be properly quantized.
            Original:  {:?}
            Quantized: {:?}",
                dht_arc_set,
                arq_set
            );
        }
    }

    let times = TelescopingTimes::historical(&topology);
    let coords = RegionCoordSetLtcs::new(times, arq_set);

    let region_set = db
        .async_reader(move |txn| {
            let sql = holochain_sqlite::sql::sql_cell::FETCH_OP_REGION;
            let mut stmt = txn.prepare_cached(sql).map_err(DatabaseError::from)?;
            let regions = coords
                .into_region_set(|(_, coords)| query_region_data(&mut stmt, &topology, coords))?;
            DatabaseResult::Ok(regions)
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
            let total_action_size: f64 = row.get("total_action_size")?;
            let total_entry_size: f64 = row.get("total_entry_size")?;
            let size = total_action_size + total_entry_size;
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

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;
    use holochain_serialized_bytes::UnsafeBytes;
    use holochain_state::prelude::StateMutationResult;
    use holochain_state::{prelude::insert_op, test_utils::test_dht_db};
    use holochain_types::fixt::*;
    use holochain_types::prelude::{DhtOp, DhtOpHashed, NewEntryAction};
    use holochain_zome_types::{AppEntryBytes, Entry};

    /// Ensure that the size reported by RegionData is "close enough" to the actual size of
    /// ops that get transferred over the wire.
    #[tokio::test(flavor = "multi_thread")]
    async fn query_region_set_diff_size() {
        let db = test_dht_db();
        let topo = Topology::standard(Timestamp::now(), Duration::ZERO);
        let strat = ArqStrat::default();
        let arcset = Arc::new(DhtArcSet::Full);

        let regions_empty = query_region_set(db.to_db(), topo.clone(), &strat, arcset.clone())
            .await
            .unwrap();
        {
            let sum: RegionData = regions_empty.regions().map(|r| r.data).sum();
            assert_eq!(sum.count, 0);
            assert_eq!(sum.size, 0);
        }

        let mk_op = |i: u8| {
            let entry = Box::new(Entry::App(AppEntryBytes(
                UnsafeBytes::from(vec![i % 10; 10_000_000])
                    .try_into()
                    .unwrap(),
            )));
            let sig = fixt::fixt!(Signature);
            let mut create = fixt::fixt!(Create);
            create.timestamp = Timestamp::now();
            let action = NewEntryAction::Create(create);
            DhtOpHashed::from_content_sync(DhtOp::StoreEntry(sig, action, entry))
        };
        let num = 100;

        let ops: Vec<_> = (0..num).map(mk_op).collect();
        let wire_bytes: usize = ops
            .clone()
            .into_iter()
            .map(|op| {
                holochain_p2p::WireDhtOpData {
                    op_data: op.into_content(),
                }
                .encode()
                .unwrap()
                .len()
            })
            .sum();

        db.test_commit(|txn| {
            for op in ops.iter() {
                insert_op(txn, op).unwrap()
            }
            StateMutationResult::Ok(())
        })
        .unwrap();

        let regions = query_region_set(db.to_db(), topo, &strat, arcset)
            .await
            .unwrap();

        let diff = regions.diff(regions_empty).unwrap().ours;
        {
            let sum: RegionData = diff.into_iter().map(|r| r.data).sum();
            assert_eq!(sum.count, num as u32);
            // 32 bytes is "close enough"
            assert!(wire_bytes as u32 - sum.size < 32 * num as u32);
        }
    }
}
