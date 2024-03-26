//! Methods for awaiting consistency between cells of the same DNA

use hc_sleuth::SleuthId;

use crate::{prelude::*, test_utils::consistency_dbs};
use std::time::Duration;

use super::*;

#[derive(derive_more::From, Debug)]
pub enum DurationOrSeconds {
    Duration(Duration),
    Seconds(u64),
}

impl DurationOrSeconds {
    pub fn into_duration(self) -> Duration {
        match self {
            Self::Duration(d) => d,
            Self::Seconds(s) => Duration::from_secs(s),
        }
    }
}

/// Wait for all cells to reach consistency
#[tracing::instrument(skip(all_cells))]
pub async fn await_consistency<'a, I: IntoIterator<Item = &'a SweetCell>>(
    timeout: impl Into<DurationOrSeconds>,
    all_cells: I,
) -> ConsistencyResult {
    consistency_advanced(timeout, all_cells.into_iter().map(|c| (c, true))).await
}

/// Wait for all cells to reach consistency,
/// with the option to specify that some cells are offline.
///
/// Cells paired with a `false` value will have their authored ops counted towards the total,
/// but not their integrated ops (since they are not online to integrate things).
/// This is useful for tests where nodes go offline.
#[tracing::instrument(skip(all_cells))]
pub async fn await_consistency_advanced<'a, I: IntoIterator<Item = (&'a SweetCell, bool)>>(
    timeout: impl Into<DurationOrSeconds>,
    all_cells: I,
) -> ConsistencyResult {
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
    consistency_dbs(&all_cell_dbs[..], timeout.into().into_duration()).await
}
