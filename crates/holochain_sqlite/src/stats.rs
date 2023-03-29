use crate::error::DatabaseError;
use rusqlite::Transaction;

pub fn get_size_on_disk(txn: Transaction) -> Result<usize, DatabaseError> {
    txn.query_row("select sum(pgsize) from dbstat", (), |r| r.get(0))
        .map_err(|e| DatabaseError::SqliteError(e))
}

pub fn get_used_size(txn: Transaction) -> Result<usize, DatabaseError> {
    txn.query_row("select sum(pgsize - unused) from dbstat", (), |r| r.get(0))
        .map_err(|e| DatabaseError::SqliteError(e))
}
