//! Free-standing operations against the `UpdatedRecord` table.

use crate::models::dht::UpdatedRecordRow;
use holo_hash::{ActionHash, EntryHash};
use sqlx::{Executor, Sqlite};

pub(crate) async fn insert_updated_record_index<'e, E>(
    executor: E,
    action_hash: &ActionHash,
    original_action_hash: &ActionHash,
    original_entry_hash: &EntryHash,
) -> sqlx::Result<()>
where
    E: Executor<'e, Database = Sqlite>,
{
    sqlx::query(
        "INSERT INTO UpdatedRecord (action_hash, original_action_hash, original_entry_hash)
         VALUES (?, ?, ?)",
    )
    .bind(action_hash.get_raw_36())
    .bind(original_action_hash.get_raw_36())
    .bind(original_entry_hash.get_raw_36())
    .execute(executor)
    .await?;
    Ok(())
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
