//! Free-standing operations against the `DeletedRecord` table.

use crate::models::dht::DeletedRecordRow;
use holo_hash::{ActionHash, EntryHash};
use sqlx::{Executor, Sqlite};

/// Parameters for inserting a row into the `DeletedRecord` index table.
pub struct InsertDeletedRecord<'a> {
    /// Hash of the Delete action (primary key).
    pub action_hash: &'a ActionHash,
    /// Hash of the action whose record is being deleted.
    pub deletes_action_hash: &'a ActionHash,
    /// Hash of the entry whose record is being deleted.
    pub deletes_entry_hash: &'a EntryHash,
}

/// Insert a row into the `DeletedRecord` index table. Returns the number of rows inserted.
pub(crate) async fn insert_deleted_record_index<'a, 'e, E>(
    executor: E,
    record: InsertDeletedRecord<'a>,
) -> sqlx::Result<u64>
where
    E: Executor<'e, Database = Sqlite>,
{
    let result = sqlx::query(
        "INSERT INTO DeletedRecord (action_hash, deletes_action_hash, deletes_entry_hash)
         VALUES (?, ?, ?)",
    )
    .bind(record.action_hash.get_raw_36())
    .bind(record.deletes_action_hash.get_raw_36())
    .bind(record.deletes_entry_hash.get_raw_36())
    .execute(executor)
    .await?;
    Ok(result.rows_affected())
}

pub(crate) async fn get_deleted_records<'e, E>(
    executor: E,
    deletes_action_hash: ActionHash,
) -> sqlx::Result<Vec<DeletedRecordRow>>
where
    E: Executor<'e, Database = Sqlite>,
{
    sqlx::query_as(
        "SELECT action_hash, deletes_action_hash, deletes_entry_hash
         FROM DeletedRecord WHERE deletes_action_hash = ?",
    )
    .bind(deletes_action_hash.get_raw_36())
    .fetch_all(executor)
    .await
}
