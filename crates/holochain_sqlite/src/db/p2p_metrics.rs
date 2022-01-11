use crate::prelude::{DatabaseError, DatabaseResult};
use crate::sql::*;
use holochain_zome_types::prelude::*;
use kitsune_p2p::event::MetricRecord;
use rusqlite::*;
use std::{
    num::TryFromIntError,
    time::{Duration, SystemTime},
};

pub fn time_to_micros(t: SystemTime) -> DatabaseResult<i64> {
    t.duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| DatabaseError::Other(e.into()))?
        .as_micros()
        .try_into()
        .map_err(|e: TryFromIntError| DatabaseError::Other(e.into()))
}

pub fn time_from_micros(micros: i64) -> DatabaseResult<SystemTime> {
    std::time::UNIX_EPOCH
        .checked_add(Duration::from_micros(micros as u64))
        .ok_or_else(|| {
            DatabaseError::Other(anyhow::anyhow!(
                "Got invalid i64 microsecond timestamp: {}",
                micros
            ))
        })
}

pub trait AsP2pMetricStoreConExt {
    fn p2p_log_metrics(&mut self, metrics: Vec<MetricRecord>) -> DatabaseResult<()>;
}

pub trait AsP2pMetricStoreTxExt {
    fn p2p_log_metrics(&self, metrics: Vec<MetricRecord>) -> DatabaseResult<()>;
}

impl AsP2pMetricStoreConExt for crate::db::PConnGuard {
    fn p2p_log_metrics(&mut self, metrics: Vec<MetricRecord>) -> DatabaseResult<()> {
        use crate::db::WriteManager;
        self.with_commit_sync(move |writer| writer.p2p_log_metrics(metrics))
    }
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
        Ok(())
    }
}
