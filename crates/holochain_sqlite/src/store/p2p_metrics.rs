use crate::error::DatabaseResult;
use crate::sql::*;
use holochain_zome_types::prelude::*;
use kitsune_p2p_types::metrics::MetricRecord;
use rusqlite::*;

#[cfg(test)]
mod p2p_metrics_test;

pub trait AsP2pMetricStoreTxExt {
    fn p2p_log_metrics(&self, metrics: Vec<MetricRecord>) -> DatabaseResult<()>;
    fn p2p_prune_metrics(&self) -> DatabaseResult<()>;
}

impl AsP2pMetricStoreTxExt for Transaction<'_> {
    fn p2p_log_metrics(&self, metrics: Vec<MetricRecord>) -> DatabaseResult<()> {
        for record in metrics {
            let kind = record.kind.to_db();
            let agent = record.agent.map(|a| a.0.clone());
            let recorded_at = record.recorded_at_utc.as_micros();
            let expires_at = record.expires_at_utc.as_micros();
            let data = record.data.to_string();
            self.execute(
                sql_p2p_metrics::INSERT,
                named_params! {
                    ":kind": kind,
                    ":agent": &agent,
                    ":recorded_at_utc_micros": recorded_at,
                    ":expires_at_utc_micros": expires_at,
                    ":data": &data,
                },
            )?;
        }
        self.p2p_prune_metrics()
    }

    fn p2p_prune_metrics(&self) -> DatabaseResult<()> {
        let now_micros = Timestamp::now().as_micros();
        self.execute(
            sql_p2p_metrics::PRUNE,
            named_params! {
                ":now_micros": now_micros,
            },
        )?;
        Ok(())
    }
}
