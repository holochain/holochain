use crate::{
    prelude::{DatabaseError, DatabaseResult},
    sql::sql_p2p_metrics,
};
use holochain_zome_types::prelude::*;
use kitsune_p2p::event::{MetricDatumKind, MetricQuery, MetricQueryAnswer};
use kitsune_p2p::*;
use rusqlite::*;
use std::{num::TryFromIntError, sync::Arc, time::Duration};

/// Record a p2p metric datum
pub fn put_metric_datum(
    txn: &mut Transaction,
    agent: Arc<KitsuneAgent>,
    metric: MetricDatumKind,
    timestamp: std::time::SystemTime,
) -> DatabaseResult<()> {
    let timestamp: u64 = timestamp
        .duration_since(std::time::UNIX_EPOCH.into())
        .map_err(|e| DatabaseError::Other(e.into()))?
        .as_nanos()
        .try_into()
        .map_err(|e: TryFromIntError| DatabaseError::Other(e.into()))?;
    txn.execute(
        sql_p2p_metrics::INSERT,
        named_params! {
            ":agent": agent.get_bytes(),
            ":metric": metric.to_string(),
            ":timestamp": timestamp
        },
    )?;
    Ok(())
}

/// Query the p2p_metrics database in a variety of ways
pub fn query_metrics(
    txn: &mut Transaction,
    query: MetricQuery,
) -> DatabaseResult<MetricQueryAnswer> {
    Ok(match query {
        MetricQuery::LastSync { agent } => {
            let timestamp: u64 = txn.query_row(
                sql_p2p_metrics::QUERY_LAST_SYNC,
                named_params! {
                    ":agent": agent.get_bytes(),
                    ":metric": MetricDatumKind::LastQuickGossip.to_string(),
                },
                |row| row.get(0),
            )?;
            dbg!(&timestamp);
            MetricQueryAnswer::LastSync(
                std::time::UNIX_EPOCH
                    .checked_add(Duration::from_nanos(timestamp))
                    .expect("weird time"),
            )
        }
        MetricQuery::Oldest {
            last_connect_error_threshold,
        } => {
            todo!()
        }
    })
}
