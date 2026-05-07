//! Free-standing operations against the `Link` table.

use crate::models::dht::LinkRow;
use holo_hash::{ActionHash, AnyLinkableHash};
use sqlx::{Executor, Sqlite};

/// Parameters for inserting a row into the `Link` index table.
pub struct InsertLink<'a> {
    /// Hash of the CreateLink action (primary key).
    pub action_hash: &'a ActionHash,
    /// DHT basis hash for this link.
    pub base_hash: &'a AnyLinkableHash,
    /// Zome index discriminant.
    pub zome_index: u8,
    /// Link type discriminant.
    pub link_type: u8,
    /// Optional tag bytes.
    pub tag: Option<&'a [u8]>,
}

/// Insert a row into the `Link` index table. Returns the number of rows inserted.
pub(crate) async fn insert_link_index<'a, 'e, E>(
    executor: E,
    link: InsertLink<'a>,
) -> sqlx::Result<u64>
where
    E: Executor<'e, Database = Sqlite>,
{
    let result = sqlx::query(
        "INSERT INTO Link (action_hash, base_hash, zome_index, link_type, tag)
         VALUES (?, ?, ?, ?, ?)",
    )
    .bind(link.action_hash.get_raw_36())
    .bind(link.base_hash.get_raw_36())
    .bind(link.zome_index as i64)
    .bind(link.link_type as i64)
    .bind(link.tag)
    .execute(executor)
    .await?;
    Ok(result.rows_affected())
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
