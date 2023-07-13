use crate::prelude::*;
use rusqlite::*;

pub type PConnInner = r2d2::PooledConnection<r2d2_sqlite::SqliteConnectionManager>;

// TODO once `conn` has been removed from the public interface, this can be made pub(crate)
/// Singleton Connection
#[derive(shrinkwraprs::Shrinkwrap)]
#[shrinkwrap(mutable)]
pub struct PConn {
    #[shrinkwrap(main_field)]
    pub inner: PConnInner,
}

impl<'e> PConn {
    pub(super) fn new(inner: PConnInner) -> Self {
        Self { inner }
    }

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
        }
        // TODO It would be possible to prevent the transaction from calling commit here if we passed a reference instead of a move.
        f(txn)
    }

    /// Run a closure, passing in a mutable reference to a read-write
    /// transaction, and commit the transaction after the closure has run.
    /// If there is a SQLite error, recover from it and re-run the closure.
    // FIXME: B-01566: implement write failure detection
    pub(super) fn execute_in_exclusive_rw_txn<E, R, F>(&'e mut self, f: F) -> Result<R, E>
    where
        E: From<DatabaseError>,
        F: 'e + FnOnce(&mut Transaction) -> Result<R, E>,
    {
        let mut txn = self
            .transaction_with_behavior(TransactionBehavior::Exclusive)
            .map_err(DatabaseError::from)?;
        let result = f(&mut txn)?;
        txn.commit().map_err(DatabaseError::from)?;
        Ok(result)
    }
}
