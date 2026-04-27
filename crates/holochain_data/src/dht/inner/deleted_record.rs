//! Free-standing operations against the `DeletedRecord` table.

use crate::models::dht::DeletedRecordRow;
use holo_hash::{ActionHash, EntryHash};
use sqlx::{Executor, Sqlite};

pub(crate) async fn insert_deleted_record_index<'e, E>(
    executor: E,
    action_hash: &ActionHash,
    deletes_action_hash: &ActionHash,
    deletes_entry_hash: &EntryHash,
) -> sqlx::Result<()>
where
    E: Executor<'e, Database = Sqlite>,
{
    sqlx::query(
        "INSERT INTO DeletedRecord (action_hash, deletes_action_hash, deletes_entry_hash)
         VALUES (?, ?, ?)",
    )
    .bind(action_hash.get_raw_36())
    .bind(deletes_action_hash.get_raw_36())
    .bind(deletes_entry_hash.get_raw_36())
    .execute(executor)
    .await?;
    Ok(())
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
