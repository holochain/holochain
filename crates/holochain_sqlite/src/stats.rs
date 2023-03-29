use crate::error::DatabaseError;
use rusqlite::Transaction;

pub fn get_size_on_disk(txn: Transaction) -> Result<usize, DatabaseError> {
    txn.execute("select sum(pgsize) from dbstat", ())
        .map_err(|e| DatabaseError::SqliteError(e))
}

pub fn get_used_size(txn: Transaction) -> Result<usize, DatabaseError> {
    txn.execute("select sum(pgsize - unused) from dbstat", ())
        .map_err(|e| DatabaseError::SqliteError(e))
}
