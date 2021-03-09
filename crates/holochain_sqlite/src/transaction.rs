use crate::rewrap_iter;
use crate::{buffer::iter::SqlIter, prelude::DatabaseResult, table::Table};
use chrono::offset::Local;
use chrono::DateTime;
use rusqlite::types::{ToSql, Value};
use rusqlite::*;
use shrinkwraprs::Shrinkwrap;

#[deprecated = "no need for read/write distinction with SQLite"]
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

pub fn delete_k<K: ToSql>(txn: &mut Transaction, table: &Table, k: &K) -> DatabaseResult<()> {
    assert!(
        table.kind().is_single(),
        "table is not single: {}",
        table.name()
    );
    let mut stmt = txn.prepare_cached(&format!("DELETE FROM {} WHERE key = ?1", table.name()))?;
    let _ = stmt.execute(params![k])?;
    Ok(())
}

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

pub fn delete_multi<K: ToSql>(txn: &mut Transaction, table: &Table, k: &K) -> DatabaseResult<()> {
    assert!(
        table.kind().is_multi(),
        "table is not multi: {}",
        table.name()
    );
    let mut stmt = txn.prepare_cached(&format!("DELETE FROM {} WHERE key = ?1", table.name()))?;
    let _ = stmt.execute(params![k])?;
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
