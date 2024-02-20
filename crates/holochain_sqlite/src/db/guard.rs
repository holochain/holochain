use crate::db::conn::PConn;
use crate::error::DatabaseResult;
use rusqlite::Transaction;
use std::ops::{Deref, DerefMut};
use std::time::Instant;
use tokio::sync::OwnedSemaphorePermit;

use super::metrics::UseTimeMetric;

pub(super) struct PConnGuard {
    conn: PConn,
    created: Instant,
    use_time_metric: UseTimeMetric,
    _permit: OwnedSemaphorePermit,
}

impl PConnGuard {
    pub(super) fn new(
        conn: PConn,
        permit: OwnedSemaphorePermit,
        use_time_metric: UseTimeMetric,
    ) -> Self {
        PConnGuard {
            conn,
            created: Instant::now(),
            use_time_metric,
            _permit: permit,
        }
    }
}

impl Deref for PConnGuard {
    type Target = PConn;

    fn deref(&self) -> &Self::Target {
        &self.conn
    }
}

impl DerefMut for PConnGuard {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.conn
    }
}

impl Drop for PConnGuard {
    fn drop(&mut self) {
        self.use_time_metric
            .record(self.created.elapsed().as_secs_f64(), &[]);
    }
}

/// Newtype to hand out connections that can only be used for running transactions
pub struct PTxnGuard(PConnGuard, Instant);

impl PTxnGuard {
    /// Start a new transaction on the inner connection held by this txn guard.
    pub fn transaction(&mut self) -> DatabaseResult<Transaction<'_>> {
        Ok(self.0.transaction()?)
    }
}

impl From<PConnGuard> for PTxnGuard {
    fn from(value: PConnGuard) -> Self {
        PTxnGuard(value, Instant::now())
    }
}

impl Drop for PTxnGuard {
    fn drop(&mut self) {
        // TODO record histogram rather than logging a warning on a fixed threshold
        let elapsed_millis = self.1.elapsed().as_millis();
        if elapsed_millis > 50 {
            tracing::warn!("PTxnGuard was held for {:?}ms", elapsed_millis);
        }
    }
}
