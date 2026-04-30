//! Free-standing operations against the `Link` table.

use crate::models::dht::LinkRow;
use holo_hash::{ActionHash, AnyLinkableHash};
use sqlx::{Executor, Sqlite};

pub(crate) async fn insert_link_index<'e, E>(
    executor: E,
    action_hash: &ActionHash,
    base_hash: &AnyLinkableHash,
    zome_index: u8,
    link_type: u8,
    tag: Option<&[u8]>,
) -> sqlx::Result<()>
where
    E: Executor<'e, Database = Sqlite>,
{
    sqlx::query(
        "INSERT INTO Link (action_hash, base_hash, zome_index, link_type, tag)
         VALUES (?, ?, ?, ?, ?)",
    )
    .bind(action_hash.get_raw_36())
    .bind(base_hash.get_raw_36())
    .bind(zome_index as i64)
    .bind(link_type as i64)
    .bind(tag)
    .execute(executor)
    .await?;
    Ok(())
}

pub(crate) async fn get_links_by_base<'e, E>(
    executor: E,
    base: AnyLinkableHash,
) -> sqlx::Result<Vec<LinkRow>>
where
    E: Executor<'e, Database = Sqlite>,
{
    sqlx::query_as(
        "SELECT action_hash, base_hash, zome_index, link_type, tag
         FROM Link WHERE base_hash = ?",
    )
    .bind(base.get_raw_36())
    .fetch_all(executor)
    .await
}
