//! Queries for the P2pMetrics store

use super::error::ConductorResult;
use holochain_types::prelude::*;
use kitsune_p2p::event::{MetricDatum, MetricQuery, MetricQueryAnswer};

/// Record a p2p metric datum
pub fn put_metric_datum(
    _env: EnvWrite,
    _agent: AgentPubKey,
    _metric: MetricDatum,
) -> ConductorResult<()> {
    todo!()
}

/// Query the p2p_metrics database in a variety of ways
pub fn query_metrics(_env: EnvWrite, _query: MetricQuery) -> ConductorResult<MetricQueryAnswer> {
    todo!()
}
