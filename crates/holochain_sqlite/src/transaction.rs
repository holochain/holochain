use crate::rewrap_iter;
use crate::{buffer::iter::SqlIter, prelude::DatabaseResult, table::Table};
use chrono::offset::Local;
use chrono::DateTime;
use rusqlite::types::{ToSql, Value};
use rusqlite::*;
use shrinkwraprs::Shrinkwrap;

/// Abstraction over readable transactions. This is a vestigal trait left over
/// from the LMDB days, where we had strict read-only transactions. While we
/// currently do have `Reader` and `Writer`, the read-only behavior is not
/// strictly enforced, and is only true by virtue of the fact that we never
/// perform write operations with a `Reader` transaction.
///
/// As we write more intricate queries this may change and we may do away with
/// our attempt at keeping read-only transactions.
pub trait Readable {
    fn get<K: ToSql>(&mut self, table: &Table, k: K) -> DatabaseResult<Option<Value>>;

    fn get_multi<K: ToSql>(&mut self, table: &Table, k: K) -> DatabaseResult<SqlIter>;

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

/// Get the value for a key (must be single table)
fn get_k<K: ToSql>(txn: &mut Transaction, table: &Table, k: K) -> DatabaseResult<Option<Value>> {
    assert!(
        table.kind().is_single(),
        "table is not single: {}",
        table.name()
    );
    let mut stmt = txn.prepare_cached(&format!(
        "SELECT key, val FROM {} WHERE key = ?1",
        table.name()
    ))?;
    Ok(stmt
        .query_row(params![k], |row| {
            // TODO: ideally we could call get_raw_unchecked to get a Value
            // and avoid cloning, but it's hard to figure out how to line up the
            // lifetime of the row with the lifetime of the transaction, or if
            // that's even possible:
            // row.get_raw_checked(1)
            row.get(1)
        })
        .optional()?)
}

/// Iterate over all rows containing this key (must be multi table)
fn get_multi<K: ToSql>(txn: &mut Transaction, table: &Table, k: K) -> DatabaseResult<SqlIter> {
    assert!(
        table.kind().is_multi(),
        "table is not multi: {}",
        table.name()
    );
    let mut stmt = txn.prepare_cached(&format!(
        "SELECT key, val FROM {} WHERE key = ?1 ORDER BY key ASC",
        table.name()
    ))?;
    let it = rewrap_iter!(stmt.query_map(params![k], |row| { Ok((row.get(0)?, row.get(1)?)) })?);
    Ok(Box::new(it))
}

/// Put a key-val pair into a database.
/// The unique index constraints particular to that kind of table will ensure
/// the proper behavior:
/// - "single" tables have unique key constraints, so an insert with an existing
///   key replaces the old value with the new
/// - "multi" tables have unique (key, val) pairs, so you can insert different
///   values under the same key, but inserting the exact same pair is a noop,
///   rather than creating duplicates.
pub(crate) fn put_kv<K: ToSql, V: ToSql>(
    txn: &mut Transaction,
    table: &Table,
    k: &K,
    v: &V,
) -> DatabaseResult<()> {
    let mut stmt = txn.prepare_cached(&format!(
        "INSERT OR REPLACE INTO {} (key, val) VALUES (?1, ?2)",
        table.name()
    ))?;
    let _ = stmt.execute(params![k, v])?;
    Ok(())
}

/// Delete all rows containing this key.
fn delete_k<K: ToSql>(txn: &mut Transaction, table: &Table, k: &K) -> DatabaseResult<()> {
    let mut stmt = txn.prepare_cached(&format!("DELETE FROM {} WHERE key = ?1", table.name()))?;
    let _ = stmt.execute(params![k])?;
    Ok(())
}

/// Delete the key-val pair with this key (must be a single table)
pub fn delete_single<K: ToSql>(txn: &mut Transaction, table: &Table, k: &K) -> DatabaseResult<()> {
    assert!(
        table.kind().is_single(),
        "table is not single: {}",
        table.name()
    );
    delete_k(txn, table, k)
}

/// Delete all items with this key (must be a multi table)
pub fn delete_multi<K: ToSql>(txn: &mut Transaction, table: &Table, k: &K) -> DatabaseResult<()> {
    assert!(
        table.kind().is_multi(),
        "table is not multi: {}",
        table.name()
    );
    delete_k(txn, table, k)
}

/// Delete a specific key-val pair (particularly useful for multi tables)
pub fn delete_kv<K: ToSql, V: ToSql>(
    txn: &mut Transaction,
    table: &Table,
    k: &K,
    v: &V,
) -> DatabaseResult<()> {
    assert!(
        table.kind().is_multi(),
        "table is not multi: {}",
        table.name()
    );
    let mut stmt = txn.prepare_cached(&format!(
        "DELETE FROM {} WHERE key = ?1 AND val = ?2",
        table.name()
    ))?;
    let _ = stmt.execute(params![k, v])?;
    Ok(())
}

/// Wrapper around `rkv::Reader`, so it can be marked as threadsafe
#[derive(Shrinkwrap)]
pub struct Reader<'env>(#[shrinkwrap(main_field)] Transaction<'env>, ReaderSpanInfo);

impl<'env> Readable for Reader<'env> {
    fn get<K: ToSql>(&mut self, table: &Table, k: K) -> DatabaseResult<Option<Value>> {
        get_k(&mut self.0, table, k)
    }

    fn get_multi<K: ToSql>(&mut self, table: &Table, k: K) -> DatabaseResult<SqlIter> {
        get_multi(&mut self.0, table, k)
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
    fn get<K: ToSql>(&mut self, table: &Table, k: K) -> DatabaseResult<Option<Value>> {
        assert!(table.kind().is_single());
        get_k(self, table, k)
    }

    fn get_multi<K: ToSql>(&mut self, table: &Table, k: K) -> DatabaseResult<SqlIter> {
        assert!(table.kind().is_multi());
        get_multi(self, table, k)
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
