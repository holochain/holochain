//! Methods for awaiting consistency between cells of the same DNA

use hc_sleuth::SleuthId;

use crate::{prelude::*, test_utils::consistency_dbs};
use std::time::Duration;

use super::*;

/// Wait for all cells to reach consistency for 10 seconds
#[macro_export]
macro_rules! consistency {
    ($secs:literal, $cells:expr) => {
        let dur = std::time::Duration::from_secs($secs);
        consistency(dur, $cells).await
    };
}

/// Wait for all cells to reach consistency for 10 seconds
pub async fn consistency_10s<'a, I: IntoIterator<Item = &'a SweetCell>>(
    all_cells: I,
) -> Result<(), String> {
    consistency(Duration::from_secs(10), all_cells).await
}

/// Wait for all cells to reach consistency for 10 seconds,
/// with the option to specify that some cells are offline.
pub async fn consistency_10s_advanced<'a, I: IntoIterator<Item = (&'a SweetCell, bool)>>(
    all_cells: I,
) -> Result<(), String> {
    consistency_advanced(Duration::from_secs(10), all_cells).await
}

/// Wait for all cells to reach consistency for 60 seconds
pub async fn consistency_60s<'a, I: IntoIterator<Item = &'a SweetCell>>(
    all_cells: I,
) -> Result<(), String> {
    consistency(Duration::from_secs(60), all_cells).await
}

/// Wait for all cells to reach consistency for 60 seconds,
/// with the option to specify that some cells are offline.
pub async fn consistency_60s_advanced<'a, I: IntoIterator<Item = (&'a SweetCell, bool)>>(
    all_cells: I,
) -> Result<(), String> {
    consistency_advanced(Duration::from_secs(60), all_cells).await
}

/// Wait for all cells to reach consistency
#[tracing::instrument(skip(all_cells))]
pub async fn consistency<'a, I: IntoIterator<Item = &'a SweetCell>>(
    timeout: Duration,
    all_cells: I,
) -> Result<(), String> {
    consistency_advanced(timeout, all_cells.into_iter().map(|c| (c, true))).await
}

/// Wait for all cells to reach consistency,
/// with the option to specify that some cells are offline.
///
/// Cells paired with a `false` value will have their authored ops counted towards the total,
/// but not their integrated ops (since they are not online to integrate things).
/// This is useful for tests where nodes go offline.
#[tracing::instrument(skip(all_cells))]
pub async fn consistency_advanced<'a, I: IntoIterator<Item = (&'a SweetCell, bool)>>(
    timeout: Duration,
    all_cells: I,
) -> Result<(), String> {
    #[allow(clippy::type_complexity)]
    let all_cell_dbs: Vec<(
        SleuthId,
        AgentPubKey,
        DbRead<DbKindAuthored>,
        Option<DbRead<DbKindDht>>,
    )> = all_cells
        .into_iter()
        .map(|(c, online)| {
            (
                c.conductor_config().sleuth_id(),
                c.agent_pubkey().clone(),
                c.authored_db().clone().into(),
                online.then(|| c.dht_db().clone().into()),
            )
        })
        .collect();
    let all_cell_dbs: Vec<_> = all_cell_dbs
        .iter()
        .map(|c| (&c.0, &c.1, &c.2, c.3.as_ref()))
        .collect();
    consistency_dbs(&all_cell_dbs[..], timeout).await
}
