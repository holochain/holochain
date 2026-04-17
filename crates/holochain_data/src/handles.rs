//! Database access handles for read and write operations.
//!
//! This module provides typed database handles that enforce read/write semantics.
//! A [`DbWrite`] handle allows both read and write operations, while a [`DbRead`]
//! handle only allows read operations. You can obtain a [`DbRead`] from a [`DbWrite`],
//! but not the reverse.

use crate::DatabaseIdentifier;
use sqlx::{Pool, Sqlite, SqliteConnection, Transaction};

/// A read-only database handle.
///
/// This handle only allows read operations against the database.
/// It is parameterized by a [`DatabaseIdentifier`] type for type safety.
#[derive(Clone, Debug)]
pub struct DbRead<I: DatabaseIdentifier> {
    pub(crate) pool: Pool<Sqlite>,
    pub(crate) identifier: I,
}

impl<I: DatabaseIdentifier> DbRead<I> {
    /// Create a new read handle.
    ///
    /// This is primarily for internal use. Users should obtain handles
    /// via [`open_db`](crate::open_db) or by converting from a [`DbWrite`] handle.
    pub(crate) fn new(pool: Pool<Sqlite>, identifier: I) -> Self {
        Self { pool, identifier }
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
    /// Call [`TxRead::commit`] or [`TxRead::rollback`] to end the
    /// transaction; dropping it without committing rolls back (a no-op
    /// for a read-only transaction, but releases the connection).
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
        Ok(TxWrite(TxRead::new(tx, self.0.identifier.clone())))
    }
}

/// A read-only database transaction handle.
///
/// Obtained from [`DbRead::begin`], or from [`TxWrite`] via [`From`] /
/// [`AsRef`] / [`AsMut`]. Exposes the same read operations as
/// [`DbRead`], but every operation runs on a single connection inside
/// a real SQL transaction — useful for multi-read snapshots. Call
/// [`TxRead::commit`] or [`TxRead::rollback`] to end the transaction;
/// dropping without committing rolls back (with a warning logged).
pub struct TxRead<I: DatabaseIdentifier> {
    // `None` once `commit` or `rollback` has taken it. Drop checks this
    // to warn about transactions abandoned without an explicit end.
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

    /// Commit the transaction.
    pub async fn commit(mut self) -> sqlx::Result<()> {
        self.tx
            .take()
            .expect("transaction already consumed")
            .commit()
            .await
    }

    /// Roll back the transaction.
    pub async fn rollback(mut self) -> sqlx::Result<()> {
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

impl<I: DatabaseIdentifier> Drop for TxRead<I> {
    fn drop(&mut self) {
        if self.tx.is_some() {
            tracing::warn!(
                database_id = self.identifier.database_id(),
                "Transaction dropped without commit or rollback; \
                 sqlx will roll back on connection return.",
            );
        }
    }
}

/// A read-write database transaction handle.
///
/// Obtained from [`DbWrite::begin`]. Exposes the same operations as
/// [`DbWrite`] and [`DbRead`], but every operation runs on a single
/// connection inside a real SQL transaction. Call [`TxWrite::commit`]
/// to persist the changes, [`TxWrite::rollback`] to discard them, or
/// drop without committing to roll back.
pub struct TxWrite<I: DatabaseIdentifier>(TxRead<I>);

impl<I: DatabaseIdentifier> TxWrite<I> {
    /// Get a reference to the database identifier.
    pub fn identifier(&self) -> &I {
        self.0.identifier()
    }

    /// Commit the transaction, persisting all changes.
    pub async fn commit(self) -> sqlx::Result<()> {
        self.0.commit().await
    }

    /// Roll back the transaction, discarding all changes.
    pub async fn rollback(self) -> sqlx::Result<()> {
        self.0.rollback().await
    }

    pub(crate) fn conn_mut(&mut self) -> &mut SqliteConnection {
        self.0.conn_mut()
    }

    pub(crate) fn tx_mut(&mut self) -> &mut Transaction<'static, Sqlite> {
        self.0.tx_mut()
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

/// Conversion from [`TxWrite`] to [`TxRead`].
impl<I: DatabaseIdentifier> From<TxWrite<I>> for TxRead<I> {
    fn from(write: TxWrite<I>) -> Self {
        write.0
    }
}

/// Borrow a [`TxRead`] from a [`TxWrite`].
impl<I: DatabaseIdentifier> AsRef<TxRead<I>> for TxWrite<I> {
    fn as_ref(&self) -> &TxRead<I> {
        &self.0
    }
}

/// Mutably borrow a [`TxRead`] from a [`TxWrite`].
///
/// Read operations on a transaction require `&mut self` (the
/// transaction owns its connection), so reading through a [`TxWrite`]
/// goes through [`AsMut`] rather than [`AsRef`].
impl<I: DatabaseIdentifier> AsMut<TxRead<I>> for TxWrite<I> {
    fn as_mut(&mut self) -> &mut TxRead<I> {
        &mut self.0
    }
}
