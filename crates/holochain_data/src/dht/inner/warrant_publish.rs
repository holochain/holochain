//! Free-standing operations against the `WarrantPublish` table.

use crate::models::dht::WarrantPublishRow;
use holo_hash::DhtOpHash;
use holochain_timestamp::Timestamp;
use sqlx::{Executor, Sqlite};

pub(crate) async fn insert_warrant_publish<'e, E>(
    executor: E,
    warrant_hash: &DhtOpHash,
    last_publish_time: Option<Timestamp>,
) -> sqlx::Result<()>
where
    E: Executor<'e, Database = Sqlite>,
{
    sqlx::query("INSERT INTO WarrantPublish (warrant_hash, last_publish_time) VALUES (?, ?)")
        .bind(warrant_hash.get_raw_36())
        .bind(last_publish_time.map(|t| t.as_micros()))
        .execute(executor)
        .await?;
    Ok(())
}

pub(crate) async fn get_warrant_publish<'e, E>(
    executor: E,
    warrant_hash: DhtOpHash,
) -> sqlx::Result<Option<WarrantPublishRow>>
where
    E: Executor<'e, Database = Sqlite>,
{
    sqlx::query_as(
        "SELECT warrant_hash, last_publish_time FROM WarrantPublish WHERE warrant_hash = ?",
    )
    .bind(warrant_hash.get_raw_36())
    .fetch_optional(executor)
    .await
}
