//! Free-standing operations against the `UpdatedRecord` table.

use crate::models::dht::UpdatedRecordRow;
use holo_hash::{ActionHash, EntryHash};
use sqlx::{Executor, Sqlite};

/// Parameters for inserting a row into the `UpdatedRecord` index table.
pub struct InsertUpdatedRecord<'a> {
    /// Hash of the Update action (primary key).
    pub action_hash: &'a ActionHash,
    /// Hash of the original action being updated.
    pub original_action_hash: &'a ActionHash,
    /// Hash of the original entry being updated.
    pub original_entry_hash: &'a EntryHash,
}

/// Insert a row into the `UpdatedRecord` index table. Returns the number of rows inserted.
pub(crate) async fn insert_updated_record_index<'a, 'e, E>(
    executor: E,
    record: InsertUpdatedRecord<'a>,
) -> sqlx::Result<u64>
where
    E: Executor<'e, Database = Sqlite>,
{
    let result = sqlx::query(
        "INSERT INTO UpdatedRecord (action_hash, original_action_hash, original_entry_hash)
         VALUES (?, ?, ?)",
    )
    .bind(record.action_hash.get_raw_36())
    .bind(record.original_action_hash.get_raw_36())
    .bind(record.original_entry_hash.get_raw_36())
    .execute(executor)
    .await?;
    Ok(result.rows_affected())
}

pub(crate) async fn get_updated_records<'e, E>(
    executor: E,
    original_action_hash: ActionHash,
) -> sqlx::Result<Vec<UpdatedRecordRow>>
where
    E: Executor<'e, Database = Sqlite>,
{
    sqlx::query_as(
        "SELECT action_hash, original_action_hash, original_entry_hash
         FROM UpdatedRecord WHERE original_action_hash = ?",
    )
    .bind(original_action_hash.get_raw_36())
    .fetch_all(executor)
    .await
}
