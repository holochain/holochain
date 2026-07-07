//! Database access handles for read and write operations.
//!
//! This module provides typed database handles that enforce read/write semantics.
//! A [`DbWrite`] handle allows both read and write operations, while a [`DbRead`]
//! handle only allows read operations. You can obtain a [`DbRead`] from a [`DbWrite`],
//! but not the reverse.

use crate::metrics::{
    create_connection_use_time_metric, create_write_txn_duration_metric, ConnectionUseTimeMetric,
    WriteTxnDurationMetric,
};
use crate::DatabaseIdentifier;
use sqlx::pool::PoolConnection;
use sqlx::{Pool, Sqlite, SqliteConnection, Transaction};
use std::ops::{Deref, DerefMut};
use std::time::Instant;

/// A read-only database handle.
///
/// This handle only allows read operations against the database.
/// It is parameterized by a [`DatabaseIdentifier`] type for type safety.
#[derive(Clone, Debug)]
pub struct DbRead<I: DatabaseIdentifier> {
    pub(crate) pool: Pool<Sqlite>,
    pub(crate) identifier: I,
    /// Records `hc.db.connections.use_time` for connections borrowed from
    /// [`pool`](Self::pool) via [`timed_conn`](Self::timed_conn).
    use_time_metric: ConnectionUseTimeMetric,
    /// Records `hc.db.write_txn.duration` when a write transaction opened from
    /// this handle is committed (see [`DbWrite::begin`] / [`TxWrite::commit`]).
    write_txn_metric: WriteTxnDurationMetric,
}

impl<I: DatabaseIdentifier> DbRead<I> {
    /// Create a new read handle.
    ///
    /// This is primarily for internal use. Users should obtain handles
    /// via [`open_db`](crate::open_db) or by converting from a [`DbWrite`] handle.
    pub(crate) fn new(pool: Pool<Sqlite>, identifier: I) -> Self {
        let use_time_metric = create_connection_use_time_metric(&identifier);
        let write_txn_metric = create_write_txn_duration_metric(&identifier);
        Self {
            pool,
            identifier,
            use_time_metric,
            write_txn_metric,
        }
    }

    /// Acquire a connection from the pool, timing its use.
    ///
    /// The returned [`TimedConn`] dereferences to a [`SqliteConnection`] and,
    /// when dropped, records the elapsed time it was held as the
    /// `hc.db.connections.use_time` metric. Read operations route through this
    /// so that connection-use is measured the same way `holochain_sqlite`
    /// measured borrowed rusqlite connections.
    pub(crate) async fn timed_conn(&self) -> sqlx::Result<TimedConn> {
        let conn = self.pool.acquire().await?;
        Ok(TimedConn::new(conn, self.use_time_metric.clone()))
    }

    /// Get a reference to the database identifier.
    pub fn identifier(&self) -> &I {
        &self.identifier
    }

    /// Get a reference to the underlying connection pool.
    pub fn pool(&self) -> &Pool<Sqlite> {
        &self.pool
    }

    /// Begin a read-only transaction.
    ///
    /// The returned [`TxRead`] exposes the same read operations as this
    /// handle but runs them on a single connection inside a SQL
    /// transaction, giving a consistent snapshot across multiple reads.
    /// Call [`TxRead::close`] to release the connection promptly;
    /// dropping has the same effect (sqlx rolls back on return).
    pub async fn begin(&self) -> sqlx::Result<TxRead<I>> {
        let tx = self.pool.begin().await?;
        Ok(TxRead::new(tx, self.identifier.clone()))
    }
}

/// A read-write database handle.
///
/// This handle allows both read and write operations against the database.
/// It can be converted into a [`DbRead`] handle for contexts that only need
/// read access.
#[derive(Clone, Debug)]
pub struct DbWrite<I: DatabaseIdentifier>(DbRead<I>);

impl<I: DatabaseIdentifier> DbWrite<I> {
    /// Create a new write handle.
    ///
    /// This is primarily for internal use. Users should obtain handles
    /// via [`open_db`](crate::open_db).
    pub(crate) fn new(pool: Pool<Sqlite>, identifier: I) -> Self {
        Self(DbRead::new(pool, identifier))
    }

    /// Get a reference to the database identifier.
    pub fn identifier(&self) -> &I {
        self.0.identifier()
    }

    /// Get a reference to the underlying connection pool.
    pub fn pool(&self) -> &Pool<Sqlite> {
        self.0.pool()
    }

    /// Begin a read-write transaction.
    ///
    /// The returned [`TxWrite`] exposes the same operations as this
    /// handle but runs them inside a single database transaction. Call
    /// [`TxWrite::commit`] to persist the changes or
    /// [`TxWrite::rollback`] to discard them; dropping without
    /// committing rolls back.
    pub async fn begin(&self) -> sqlx::Result<TxWrite<I>> {
        let tx = self.pool().begin().await?;
        Ok(TxWrite {
            inner: TxRead::new(tx, self.0.identifier.clone()),
            start: std::time::Instant::now(),
            write_txn_metric: self.0.write_txn_metric.clone(),
        })
    }
}

/// A read-only database transaction handle.
///
/// Obtained from [`DbRead::begin`], or from [`TxWrite`] via [`From`] /
/// [`AsRef`] / [`AsMut`]. Exposes the same read operations as
/// [`DbRead`], but every operation runs on a single connection inside
/// a real SQL transaction — useful for multi-read snapshots. Call
/// [`TxRead::close`] to release the connection, or simply drop it
/// (sqlx rolls back on connection return).
pub struct TxRead<I: DatabaseIdentifier> {
    // `None` once `close` has taken it. Drop falls back to sqlx's
    // automatic rollback on connection return.
    tx: Option<Transaction<'static, Sqlite>>,
    identifier: I,
}

impl<I: DatabaseIdentifier> TxRead<I> {
    pub(crate) fn new(tx: Transaction<'static, Sqlite>, identifier: I) -> Self {
        Self {
            tx: Some(tx),
            identifier,
        }
    }

    /// Get a reference to the database identifier.
    pub fn identifier(&self) -> &I {
        &self.identifier
    }

    /// Close the transaction and release the connection back to the pool.
    ///
    /// Rolls back the underlying transaction; for a read-only transaction
    /// there is nothing to persist, so this is equivalent to dropping
    /// except that any rollback error is surfaced.
    pub async fn close(mut self) -> sqlx::Result<()> {
        self.tx
            .take()
            .expect("transaction already consumed")
            .rollback()
            .await
    }

    pub(crate) fn conn_mut(&mut self) -> &mut SqliteConnection {
        self.tx.as_mut().expect("transaction already consumed")
    }

    pub(crate) fn tx_mut(&mut self) -> &mut Transaction<'static, Sqlite> {
        self.tx.as_mut().expect("transaction already consumed")
    }
}

/// A read-write database transaction handle.
///
/// Obtained from [`DbWrite::begin`]. Exposes the same operations as
/// [`DbWrite`] and [`DbRead`], but every operation runs on a single
/// connection inside a real SQL transaction. Call [`TxWrite::commit`]
/// to persist the changes, [`TxWrite::rollback`] to discard them, or
/// drop without committing to roll back.
pub struct TxWrite<I: DatabaseIdentifier> {
    inner: TxRead<I>,
    /// When the transaction was opened, used to record `hc.db.write_txn.duration`
    /// on commit.
    start: std::time::Instant,
    write_txn_metric: WriteTxnDurationMetric,
}

impl<I: DatabaseIdentifier> TxWrite<I> {
    /// Get a reference to the database identifier.
    pub fn identifier(&self) -> &I {
        self.inner.identifier()
    }

    /// Commit the transaction, persisting all changes.
    pub async fn commit(mut self) -> sqlx::Result<()> {
        self.inner
            .tx
            .take()
            .expect("transaction already consumed")
            .commit()
            .await?;
        self.write_txn_metric
            .record(self.start.elapsed().as_secs_f64());
        Ok(())
    }

    /// Roll back the transaction, discarding all changes.
    pub async fn rollback(mut self) -> sqlx::Result<()> {
        self.inner
            .tx
            .take()
            .expect("transaction already consumed")
            .rollback()
            .await
    }

    pub(crate) fn conn_mut(&mut self) -> &mut SqliteConnection {
        self.inner.conn_mut()
    }

    pub(crate) fn tx_mut(&mut self) -> &mut Transaction<'static, Sqlite> {
        self.inner.tx_mut()
    }
}

/// Conversion from [`DbWrite`] to [`DbRead`].
///
/// This allows a write handle to be used in contexts that only require read access.
impl<I: DatabaseIdentifier> From<DbWrite<I>> for DbRead<I> {
    fn from(write: DbWrite<I>) -> Self {
        write.0
    }
}

/// Borrow a [`DbRead`] from a [`DbWrite`].
impl<I: DatabaseIdentifier> AsRef<DbRead<I>> for DbWrite<I> {
    fn as_ref(&self) -> &DbRead<I> {
        &self.0
    }
}

/// Identity borrow on [`DbRead`]. Lets generic call sites accept either
/// `DbRead<I>` or `DbWrite<I>` through `AsRef<DbRead<I>>`.
impl<I: DatabaseIdentifier> AsRef<DbRead<I>> for DbRead<I> {
    fn as_ref(&self) -> &DbRead<I> {
        self
    }
}

/// Conversion from [`TxWrite`] to [`TxRead`].
impl<I: DatabaseIdentifier> From<TxWrite<I>> for TxRead<I> {
    fn from(write: TxWrite<I>) -> Self {
        write.inner
    }
}

/// Borrow a [`TxRead`] from a [`TxWrite`].
impl<I: DatabaseIdentifier> AsRef<TxRead<I>> for TxWrite<I> {
    fn as_ref(&self) -> &TxRead<I> {
        &self.inner
    }
}

/// Mutably borrow a [`TxRead`] from a [`TxWrite`].
///
/// Read operations on a transaction require `&mut self` (the
/// transaction owns its connection), so reading through a [`TxWrite`]
/// goes through [`AsMut`] rather than [`AsRef`].
impl<I: DatabaseIdentifier> AsMut<TxRead<I>> for TxWrite<I> {
    fn as_mut(&mut self) -> &mut TxRead<I> {
        &mut self.inner
    }
}

/// A pooled connection whose use is timed.
///
/// Obtained from [`DbRead::timed_conn`]. Dereferences to the borrowed
/// [`SqliteConnection`] (so it can be passed to any `sqlx::Executor`-bound
/// operation), and records the time it was held as the
/// `hc.db.connections.use_time` metric when dropped. The connection is
/// returned to the pool by the inner [`PoolConnection`]'s own drop.
pub(crate) struct TimedConn {
    conn: PoolConnection<Sqlite>,
    use_time_metric: ConnectionUseTimeMetric,
    acquired_at: Instant,
}

impl TimedConn {
    fn new(conn: PoolConnection<Sqlite>, use_time_metric: ConnectionUseTimeMetric) -> Self {
        Self {
            conn,
            use_time_metric,
            acquired_at: Instant::now(),
        }
    }
}

impl Deref for TimedConn {
    type Target = SqliteConnection;

    fn deref(&self) -> &Self::Target {
        &self.conn
    }
}

impl DerefMut for TimedConn {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.conn
    }
}

impl Drop for TimedConn {
    fn drop(&mut self) {
        self.use_time_metric
            .record(self.acquired_at.elapsed().as_secs_f64());
    }
}
