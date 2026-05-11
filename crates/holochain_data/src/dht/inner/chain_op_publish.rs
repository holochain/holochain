//! Free-standing operations against the `ChainOpPublish` table.

use crate::models::dht::ChainOpPublishRow;
use holo_hash::DhtOpHash;
use holochain_timestamp::Timestamp;
use sqlx::{Executor, Sqlite};

pub(crate) async fn insert_chain_op_publish<'e, E>(
    executor: E,
    op_hash: &DhtOpHash,
    last_publish_time: Option<Timestamp>,
    receipts_complete: Option<bool>,
    withhold_publish: Option<bool>,
) -> sqlx::Result<()>
where
    E: Executor<'e, Database = Sqlite>,
{
    sqlx::query(
        "INSERT INTO ChainOpPublish (op_hash, last_publish_time, receipts_complete, withhold_publish)
         VALUES (?, ?, ?, ?)",
    )
    .bind(op_hash.get_raw_36())
    .bind(last_publish_time.map(|t| t.as_micros()))
    .bind(receipts_complete.map(|b| b as i64))
    .bind(withhold_publish.map(|b| b as i64))
    .execute(executor)
    .await?;
    Ok(())
}

pub(crate) async fn get_chain_op_publish<'e, E>(
    executor: E,
    op_hash: DhtOpHash,
) -> sqlx::Result<Option<ChainOpPublishRow>>
where
    E: Executor<'e, Database = Sqlite>,
{
    sqlx::query_as(
        "SELECT op_hash, last_publish_time, receipts_complete, withhold_publish
         FROM ChainOpPublish WHERE op_hash = ?",
    )
    .bind(op_hash.get_raw_36())
    .fetch_optional(executor)
    .await
}

/// Mark receipts as complete for the given op. Receipts can never transition
/// back to incomplete, so no flag is needed.
pub(crate) async fn set_receipts_complete<'e, E>(
    executor: E,
    op_hash: &DhtOpHash,
) -> sqlx::Result<u64>
where
    E: Executor<'e, Database = Sqlite>,
{
    let result = sqlx::query("UPDATE ChainOpPublish SET receipts_complete = 1 WHERE op_hash = ?")
        .bind(op_hash.get_raw_36())
        .execute(executor)
        .await?;
    Ok(result.rows_affected())
}
