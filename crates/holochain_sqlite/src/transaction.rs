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
    let mut stmt = txn.prepare_cached(&format!(
        "SELECT key, val FROM {} WHERE key = ?1",
        table.name()
    ))?;
    Ok(stmt
        .query_row(params![k.as_ref()], |row| {
            // TODO: ideally we could call get_raw_unchecked to get a Value
            // and avoid cloning, but it's hard to figure out how to line up the
            // lifetime of the row with the lifetime of the transaction, or if
            // that's even possible:
            // row.get_raw_checked(1)
            row.get(1)
        })
        .optional()?)
}

pub(crate) fn put_kv<K: ToSql, V: ToSql>(
    txn: &mut Transaction,
    table: &Table,
    k: &K,
    v: &V,
) -> DatabaseResult<()> {
    let mut stmt = txn.prepare_cached(&format!(
        "INSERT INTO {} (key, val) VALUES (?1, ?2)",
        table.name()
    ))?;
    let _ = stmt.execute(params![k, v])?;
    Ok(())
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

impl<'env> Readable for Reader<'env> {
    fn get<K: AsRef<[u8]>>(&mut self, table: &Table, k: K) -> DatabaseResult<Option<Value>> {
        get_kv(&mut self.0, table, k)
    }

    fn iter_start(&mut self, table: &Table) -> DatabaseResult<SqlIter> {
        Ok(Box::new(rewrap_iter!(&mut self
            .0
            .prepare_cached(&format!(
                "SELECT key, val FROM {} ORDER BY key ASC",
                table.name()
            ))?
            .query_map(NO_PARAMS, |row| {
                Ok((row.get(0)?, Some(row.get(1)?)))
            })?)))
    }

    fn iter_end(&mut self, table: &Table) -> DatabaseResult<SqlIter> {
        Ok(Box::new(rewrap_iter!(&mut self
            .0
            .prepare_cached(&format!(
                "SELECT key, val FROM {} ORDER BY key DESC",
                table.name()
            ))?
            .query_map(NO_PARAMS, |row| {
                Ok((row.get(0)?, Some(row.get(1)?)))
            })?)))
    }

    fn iter_from<K: ToSql>(&mut self, table: &Table, k: &K) -> DatabaseResult<SqlIter> {
        Ok(Box::new(rewrap_iter!(&mut self
            .0
            .prepare_cached(&format!(
                "SELECT key, val FROM {} WHERE key >= ?1 ORDER BY key ASC",
                table.name()
            ))?
            .query_map(params![k], |row| {
                Ok((row.get(0)?, Some(row.get(1)?)))
            })?)))
    }
}

pub fn get_0<'t>(reader: &'t mut Reader<'t>) -> &'t mut Transaction<'t> {
    &mut reader.0
}

impl<'env> From<Transaction<'env>> for Reader<'env> {
    fn from(r: Transaction<'env>) -> Self {
        Self(r, ReaderSpanInfo::new())
    }
}

pub type Writer<'t> = Transaction<'t>;

// XXX: this is copy-pasted from the Reader impl because I couldn't find an easy
// way to abstract the `self` vs `self.0` difference between the two
impl<'env> Readable for Writer<'env> {
    fn get<K: AsRef<[u8]>>(&mut self, table: &Table, k: K) -> DatabaseResult<Option<Value>> {
        get_kv(self, table, k)
    }

    fn iter_start(&mut self, table: &Table) -> DatabaseResult<SqlIter> {
        Ok(Box::new(rewrap_iter!(self
            .prepare_cached(&format!(
                "SELECT key, val FROM {} ORDER BY key ASC",
                table.name()
            ))?
            .query_map(NO_PARAMS, |row| {
                Ok((row.get(0)?, Some(row.get(1)?)))
            })?)))
    }

    fn iter_end(&mut self, table: &Table) -> DatabaseResult<SqlIter> {
        Ok(Box::new(rewrap_iter!(self
            .prepare_cached(&format!(
                "SELECT key, val FROM {} ORDER BY key DESC",
                table.name()
            ))?
            .query_map(NO_PARAMS, |row| {
                Ok((row.get(0)?, Some(row.get(1)?)))
            })?)))
    }

    fn iter_from<K: ToSql>(&mut self, table: &Table, k: &K) -> DatabaseResult<SqlIter> {
        Ok(Box::new(rewrap_iter!(self
            .prepare_cached(&format!(
                "SELECT key, val FROM {} WHERE key >= ?1 ORDER BY key ASC",
                table.name()
            ))?
            .query_map(params![k], |row| {
                Ok((row.get(0)?, Some(row.get(1)?)))
            })?)))
    }
}
