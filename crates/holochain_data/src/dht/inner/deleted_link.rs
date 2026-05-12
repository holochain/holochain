//! Free-standing operations against the `DeletedLink` table.

use crate::models::dht::DeletedLinkRow;
use holo_hash::ActionHash;
use sqlx::{Executor, Sqlite};

/// Parameters for inserting a row into the `DeletedLink` index table.
pub struct InsertDeletedLink<'a> {
    /// Hash of the DeleteLink action (primary key).
    pub action_hash: &'a ActionHash,
    /// Hash of the CreateLink action being deleted.
    pub create_link_hash: &'a ActionHash,
}

/// Insert a row into the `DeletedLink` index table. Returns the number of rows inserted.
pub(crate) async fn insert_deleted_link_index<'a, 'e, E>(
    executor: E,
    link: InsertDeletedLink<'a>,
) -> sqlx::Result<u64>
where
    E: Executor<'e, Database = Sqlite>,
{
    let result =
        sqlx::query("INSERT INTO DeletedLink (action_hash, create_link_hash) VALUES (?, ?)")
            .bind(link.action_hash.get_raw_36())
            .bind(link.create_link_hash.get_raw_36())
            .execute(executor)
            .await?;
    Ok(result.rows_affected())
}

pub(crate) async fn get_deleted_links<'e, E>(
    executor: E,
    create_link_hash: ActionHash,
) -> sqlx::Result<Vec<DeletedLinkRow>>
where
    E: Executor<'e, Database = Sqlite>,
{
    sqlx::query_as(
        "SELECT action_hash, create_link_hash FROM DeletedLink WHERE create_link_hash = ?",
    )
    .bind(create_link_hash.get_raw_36())
    .fetch_all(executor)
    .await
}
