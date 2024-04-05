use crate::prelude::*;
use holochain_util::time::log_elapsed;
use rusqlite::*;
use std::ops::{Deref, DerefMut};
use tracing::instrument;

pub(super) type PConnInner = r2d2::PooledConnection<r2d2_sqlite::SqliteConnectionManager>;

/// Singleton Connection
pub(super) struct PConn {
    inner: PConnInner,
}

impl Deref for PConn {
    type Target = PConnInner;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl DerefMut for PConn {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}

impl<'e> PConn {
    pub(super) fn new(inner: PConnInner) -> Self {
        Self { inner }
    }

    #[tracing::instrument(skip_all)]
    pub(super) fn execute_in_read_txn<E, R, F>(&'e mut self, f: F) -> Result<R, E>
    where
        E: From<DatabaseError>,
        F: 'e + FnOnce(Transaction) -> Result<R, E>,
    {
        let start = std::time::Instant::now();

        let txn = self.transaction().map_err(DatabaseError::from)?;
        if start.elapsed().as_millis() > 100 {
            let s = tracing::debug_span!("timing_1");
            s.in_scope(
                || tracing::debug!(file = %file!(), line = %line!(), time = ?start.elapsed()),
            );
        } else {
            tracing::trace!("txn acquired");
        }
        // TODO It would be possible to prevent the transaction from calling commit here if we passed a reference instead of a move.
        {
            let start = tokio::time::Instant::now();
            let ret = f(txn);
            log_elapsed([10, 20, 50], start, "execute_in_read_txn:closure");
            ret
        }
    }

    /// Run a closure, passing in a mutable reference to a read-write
    /// transaction, and commit the transaction after the closure has run.
    /// If there is a SQLite error, recover from it and re-run the closure.
    // FIXME: B-01566: implement write failure detection
    #[tracing::instrument(skip_all)]
    pub(super) fn execute_in_exclusive_rw_txn<E, R, F>(&'e mut self, f: F) -> Result<R, E>
    where
        E: From<DatabaseError>,
        F: 'e + FnOnce(&mut Transaction) -> Result<R, E>,
    {
        tracing::trace!("entered execute_in_exclusive_rw_txn");

        let mut txn = {
            let start = tokio::time::Instant::now();
            let txn = self
                .transaction_with_behavior(TransactionBehavior::Exclusive)
                .map_err(DatabaseError::from)?;
            log_elapsed(
                [10, 100, 1000],
                start,
                "execute_in_exclusive_rw_txn:transaction_with_behavior",
            );
            txn
        };

        let result = {
            let start = tokio::time::Instant::now();
            let result = f(&mut txn)?;
            log_elapsed(
                [10, 100, 1000],
                start,
                "closure in execute_in_exclusive_rw_txn",
            );
            result
        };

        {
            let start = tokio::time::Instant::now();
            txn.commit().map_err(DatabaseError::from)?;
            log_elapsed([10, 100, 1000], start, "execute_in_exclusive_rw_txn:commit");
        }
        Ok(result)
    }
}
