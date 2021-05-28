use crate::{prelude::DatabaseResult, sql::sql_p2p_metrics};
use holo_hash::AgentPubKey;
use kitsune_p2p::event::MetricDatumKind;
use rusqlite::*;

/// Record a p2p metric datum
pub fn put_metric_datum(
    txn: Transaction,
    agent: AgentPubKey,
    metric: MetricDatumKind,
    timestamp: std::time::SystemTime,
) -> DatabaseResult<()> {
    // let t: u64 = timestamp
    //     .duration_since(std::time::UNIX_EPOCH.into())?
    //     .as_millis()
    //     .try_into()?;
    txn.execute(
        sql_p2p_metrics::INSERT,
        named_params! {
            ":agent": agent,
            ":metric": metric.to_string(),
            ":timestamp": timestamp
        },
    )?;
    Ok(())
}

/// Query the p2p_metrics database in a variety of ways
pub fn query_metrics(_env: EnvWrite, _query: MetricQuery) -> ConductorResult<MetricQueryAnswer> {
    todo!()
}
