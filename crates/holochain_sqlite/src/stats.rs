use rusqlite::OptionalExtension;
use crate::{
    db::{DbKindT, Txn},
    error::DatabaseError,
};

pub fn get_size_on_disk<K: DbKindT>(txn: &Txn<K>) -> Result<usize, DatabaseError> {
    Ok(txn
        .query_row("select sum(pgsize) from dbstat", (), |r| r.get(0))
        .optional()
        .map_err(DatabaseError::SqliteError)?
        .unwrap_or_default())
}

pub fn get_used_size<K: DbKindT>(txn: &Txn<K>) -> Result<usize, DatabaseError> {
    Ok(txn
        .query_row("select sum(pgsize - unused) from dbstat", (), |r| r.get(0))
        .optional()
        .map_err(DatabaseError::SqliteError)?
        .unwrap_or_default())
}
