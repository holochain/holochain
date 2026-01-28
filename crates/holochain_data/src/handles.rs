//! Database access handles for read and write operations.
//!
//! This module provides typed database handles that enforce read/write semantics.
//! A [`DbWrite`] handle allows both read and write operations, while a [`DbRead`]
//! handle only allows read operations. You can obtain a [`DbRead`] from a [`DbWrite`],
//! but not the reverse.

use crate::DatabaseIdentifier;
use sqlx::{Pool, Sqlite};

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
