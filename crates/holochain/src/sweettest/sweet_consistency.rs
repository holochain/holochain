//! Methods for awaiting consistency between cells of the same DNA

use crate::{
    prelude::*,
    test_utils::{wait_for_integration_diff, ConsistencyConditions, ConsistencyResult},
};
use std::time::Duration;

use super::*;

/// A duration expressed properly, or just as seconds
#[derive(derive_more::From, Debug)]
pub enum DurationOrSeconds {
    /// Proper duration
    Duration(Duration),
    /// Just seconds
    Seconds(u64),
}

impl DurationOrSeconds {
    /// Get the proper duration
    pub fn into_duration(self) -> Duration {
        match self {
            Self::Duration(d) => d,
            Self::Seconds(s) => Duration::from_secs(s),
        }
    }
}

/// Wait for all cells to reach consistency
#[cfg_attr(feature = "instrument", tracing::instrument(skip_all))]
pub async fn await_consistency<'a, I: IntoIterator<Item = &'a SweetCell>>(
    timeout: impl Into<DurationOrSeconds>,
    all_cells: I,
) -> ConsistencyResult {
    await_consistency_advanced(timeout, (), all_cells.into_iter().map(|c| (c, true))).await
}

/// Wait for all cells to reach consistency
#[cfg_attr(feature = "instrument", tracing::instrument(skip_all))]
pub async fn await_consistency_conditional<'a, I: IntoIterator<Item = &'a SweetCell>>(
    timeout: impl Into<DurationOrSeconds>,
    conditions: impl Into<ConsistencyConditions>,
    all_cells: I,
) -> ConsistencyResult {
    await_consistency_advanced(
        timeout,
        conditions,
        all_cells.into_iter().map(|c| (c, true)),
    )
    .await
}

/// Wait for all cells to reach consistency,
/// with the option to specify that some cells are offline.
///
/// Cells paired with a `false` value will have their authored ops counted towards the total,
/// but not their integrated ops (since they are not online to integrate things).
/// This is useful for tests where nodes go offline.
#[cfg_attr(feature = "instrument", tracing::instrument(skip_all))]
pub async fn await_consistency_advanced<'a, I: IntoIterator<Item = (&'a SweetCell, bool)>>(
    timeout: impl Into<DurationOrSeconds>,
    conditions: impl Into<ConsistencyConditions>,
    all_cells: I,
) -> ConsistencyResult {
    #[allow(clippy::type_complexity)]
    let all_cell_dbs: Vec<(
        AgentPubKey,
        DbRead<DbKindAuthored>,
        Option<DbRead<DbKindDht>>,
    )> = all_cells
        .into_iter()
        .map(|(c, online)| {
            (
                c.agent_pubkey().clone(),
                c.authored_db().clone().into(),
                online.then(|| c.dht_db().clone().into()),
            )
        })
        .collect();
    let all_cell_dbs: Vec<_> = all_cell_dbs
        .iter()
        .map(|c| (&c.0, &c.1, c.2.as_ref()))
        .collect();
    wait_for_integration_diff(
        &all_cell_dbs[..],
        timeout.into().into_duration(),
        conditions.into(),
    )
    .await
}
