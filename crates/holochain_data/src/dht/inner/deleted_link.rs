//! Free-standing operations against the `DeletedLink` table.

use crate::models::dht::DeletedLinkRow;
use holo_hash::ActionHash;
use sqlx::{Executor, Sqlite};

pub(crate) async fn insert_deleted_link_index<'e, E>(
    executor: E,
    action_hash: &ActionHash,
    create_link_hash: &ActionHash,
) -> sqlx::Result<()>
where
    E: Executor<'e, Database = Sqlite>,
{
    sqlx::query("INSERT INTO DeletedLink (action_hash, create_link_hash) VALUES (?, ?)")
        .bind(action_hash.get_raw_36())
        .bind(create_link_hash.get_raw_36())
        .execute(executor)
        .await?;
    Ok(())
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
