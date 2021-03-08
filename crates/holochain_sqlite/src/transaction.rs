//! This module is just wrappers around the rkv transaction representations.
//! They are necessary/useful for a few reasons:
//! - Reader is not marked Send + Sync in rkv, but we must mark it such to make
//!     use of the threadsafe read-only transactions provided by the MDB_NOTLS flag
//! - We can upgrade some error types from rkv::StoreError, which does not implement
//!     std::error::Error, into error types that do

use crate::rewrap_iter;
use crate::{buffer::iter::SqlIter, error::DatabaseError, prelude::DatabaseResult, table::Table};
use chrono::offset::Local;
use chrono::DateTime;
use derive_more::From;
use rusqlite::types::Value;
use rusqlite::*;
use shrinkwraprs::Shrinkwrap;

#[deprecated = "no need for read/write distinction with SQLite"]
pub trait Readable {
    fn get<K: AsRef<[u8]>>(&mut self, table: &Table, k: K) -> DatabaseResult<Option<Value>>;

    fn iter_start(&mut self, table: &Table) -> DatabaseResult<SqlIter>;

    fn iter_end(&mut self, table: &Table) -> DatabaseResult<SqlIter>;

    fn iter_from<K: ToSql>(&mut self, table: &Table, k: &K) -> DatabaseResult<SqlIter>;
}

struct ReaderSpanInfo {
    // Using a chrono timestamp here because we need duration operations
    start_time: DateTime<Local>,
}

impl ReaderSpanInfo {
    pub fn new() -> Self {
        Self {
            start_time: Local::now(),
        }
    }
}

impl Drop for ReaderSpanInfo {
    fn drop(&mut self) {
        let ms = Local::now()
            .signed_duration_since(self.start_time)
            .num_milliseconds();
        if ms >= 100 {
            tracing::warn!("long-lived reader: {} ms", ms);
        }
    }
}

fn get_kv<K: AsRef<[u8]>>(
    txn: &mut Transaction,
    table: &Table,
    k: K,
) -> DatabaseResult<Option<Value>> {
    let mut stmt = txn.prepare_cached("SELECT key, val FROM ?1 WHERE key = ?2")?;
    Ok(stmt
        .query_row(params![&table.name(), k.as_ref()], |row| {
            // TODO: ideally we could call get_raw_unchecked to get a Value
            // and avoid cloning, but it's hard to figure out how to line up the
            // lifetime of the row with the lifetime of the transaction, or if
            // that's even possible:
            // row.get_raw_checked(1)
            row.get(1)
        })
        .optional()?)
}

/// Wrapper around `rkv::Reader`, so it can be marked as threadsafe
#[derive(Shrinkwrap)]
pub struct Reader<'env>(#[shrinkwrap(main_field)] Transaction<'env>, ReaderSpanInfo);

/// If MDB_NOTLS env flag is set, then read-only transactions are threadsafe
/// and we can mark them as such
#[cfg(feature = "lmdb_no_tls")]
unsafe impl<'env> Send for Reader<'env> {}

/// If MDB_NOTLS env flag is set, then read-only transactions are threadsafe
/// and we can mark them as such
#[cfg(feature = "lmdb_no_tls")]
unsafe impl<'env> Sync for Reader<'env> {}

macro_rules! impl_readable {
    ($t:ident) => {
        impl<'env> Readable for $t<'env> {
            fn get<K: AsRef<[u8]>>(
                &mut self,
                table: &Table,
                k: K,
            ) -> DatabaseResult<Option<Value>> {
                get_kv(&mut self.0, table, k)
            }

            fn iter_start(&mut self, table: &Table) -> DatabaseResult<SqlIter> {
                Ok(Box::new(rewrap_iter!(self
                    .0
                    .prepare_cached("SELECT key, val FROM ?1 ORDER BY key ASC")?
                    .query_map(params![&table.name()], |row| {
                        Ok((row.get(0)?, Some(row.get(1)?)))
                    })?)))
            }

            fn iter_end(&mut self, table: &Table) -> DatabaseResult<SqlIter> {
                Ok(Box::new(rewrap_iter!(self
                    .0
                    .prepare_cached("SELECT key, val FROM ?1 ORDER BY key DESC")?
                    .query_map(params![&table.name()], |row| {
                        Ok((row.get(0)?, Some(row.get(1)?)))
                    })?)))
            }

            fn iter_from<K: ToSql>(&mut self, table: &Table, k: &K) -> DatabaseResult<SqlIter> {
                Ok(Box::new(rewrap_iter!(self
                    .0
                    .prepare_cached("SELECT key, val FROM ?1 WHERE key >= ?2 ORDER BY key ASC")?
                    .query_map(params![&table.name(), k], |row| {
                        Ok((row.get(0)?, Some(row.get(1)?)))
                    })?)))
            }
        }
    };
}

impl_readable!(Reader);
impl_readable!(Writer);

impl<'env> From<Transaction<'env>> for Reader<'env> {
    fn from(r: Transaction<'env>) -> Self {
        Self(r, ReaderSpanInfo::new())
    }
}

/// Wrapper around `rkv::Writer`, which lifts some of the return values to types recognized by this crate,
/// rather than the rkv-specific values
#[derive(From, Shrinkwrap)]
#[shrinkwrap(mutable, unsafe_ignore_visibility)]
pub struct Writer<'env>(Transaction<'env>);

impl<'env> Writer<'env> {
    /// This override exists solely to raise the Error from the rkv::StoreError,
    /// which does not implement std::error::Error, into a DatabaseError, which does.
    pub fn commit(self) -> Result<(), DatabaseError> {
        todo!("sqlite")
    }
}
