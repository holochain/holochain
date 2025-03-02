use crate::error::DatabaseError;
use rusqlite::Transaction;

pub fn get_size_on_disk(txn: Transaction) -> Result<usize, DatabaseError> {
    Ok(txn
        .query_row("select sum(pgsize) from dbstat", (), |r| {
            r.get::<_, Option<usize>>(0)
        })
        .map_err(DatabaseError::SqliteError)?
        .unwrap_or_default())
}

pub fn get_used_size(txn: Transaction) -> Result<usize, DatabaseError> {
    Ok(txn
        .query_row("select sum(pgsize - unused) from dbstat", (), |r| {
            r.get::<_, Option<usize>>(0)
        })
        .map_err(DatabaseError::SqliteError)?
        .unwrap_or_default())
}
