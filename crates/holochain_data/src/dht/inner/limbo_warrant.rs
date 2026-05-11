//! Free-standing operations against the `LimboWarrant` table.

use crate::models::dht::LimboWarrantRow;
use holo_hash::{AgentPubKey, DhtOpHash};
use holochain_timestamp::Timestamp;
use sqlx::{Executor, Sqlite};

/// Parameters for inserting a row into `LimboWarrant`.
pub struct InsertLimboWarrant<'a> {
    /// DHT op hash (primary key).
    pub hash: &'a DhtOpHash,
    /// Agent pub key of the warrant author.
    pub author: &'a AgentPubKey,
    /// Microsecond authoring timestamp.
    pub timestamp: Timestamp,
    /// Agent pub key of the warrantee (also serves as the DHT basis).
    pub warrantee: &'a AgentPubKey,
    /// Serialized `WarrantProof` blob.
    pub proof: &'a [u8],
    /// Numeric storage center derived from the warrantee.
    pub storage_center_loc: u32,
    /// Microsecond timestamp at which the warrant was received.
    pub when_received: Timestamp,
    /// Wire-size of the warrant in bytes.
    pub serialized_size: u32,
}

pub(crate) async fn insert_limbo_warrant<'a, 'e, E>(
    executor: E,
    w: InsertLimboWarrant<'a>,
) -> sqlx::Result<()>
where
    E: Executor<'e, Database = Sqlite>,
{
    sqlx::query(
        "INSERT INTO LimboWarrant
            (hash, author, timestamp, warrantee, proof, storage_center_loc,
             when_received, serialized_size)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(w.hash.get_raw_36())
    .bind(w.author.get_raw_36())
    .bind(w.timestamp.as_micros())
    .bind(w.warrantee.get_raw_36())
    .bind(w.proof)
    .bind(w.storage_center_loc as i64)
    .bind(w.when_received.as_micros())
    .bind(w.serialized_size as i64)
    .execute(executor)
    .await?;
    Ok(())
}

pub(crate) async fn get_limbo_warrant<'e, E>(
    executor: E,
    hash: DhtOpHash,
) -> sqlx::Result<Option<LimboWarrantRow>>
where
    E: Executor<'e, Database = Sqlite>,
{
    sqlx::query_as(
        "SELECT hash, author, timestamp, warrantee, proof, storage_center_loc,
                sys_validation_status, abandoned_at, when_received,
                sys_validation_attempts, last_validation_attempt, serialized_size
         FROM LimboWarrant WHERE hash = ?",
    )
    .bind(hash.get_raw_36())
    .fetch_optional(executor)
    .await
}

pub(crate) async fn limbo_warrants_pending_sys_validation<'e, E>(
    executor: E,
    limit: u32,
) -> sqlx::Result<Vec<LimboWarrantRow>>
where
    E: Executor<'e, Database = Sqlite>,
{
    sqlx::query_as(
        "SELECT * FROM LimboWarrant
         WHERE sys_validation_status IS NULL AND abandoned_at IS NULL
         ORDER BY sys_validation_attempts, when_received
         LIMIT ?",
    )
    .bind(limit as i64)
    .fetch_all(executor)
    .await
}

pub(crate) async fn limbo_warrants_ready_for_integration<'e, E>(
    executor: E,
    limit: u32,
) -> sqlx::Result<Vec<LimboWarrantRow>>
where
    E: Executor<'e, Database = Sqlite>,
{
    sqlx::query_as(
        "SELECT * FROM LimboWarrant
         WHERE abandoned_at IS NOT NULL OR sys_validation_status IN (1, 2)
         ORDER BY when_received
         LIMIT ?",
    )
    .bind(limit as i64)
    .fetch_all(executor)
    .await
}

pub(crate) async fn set_sys_validation_status<'e, E>(
    executor: E,
    hash: &DhtOpHash,
    status: Option<i64>,
) -> sqlx::Result<u64>
where
    E: Executor<'e, Database = Sqlite>,
{
    let result = sqlx::query(
        "UPDATE LimboWarrant SET sys_validation_status = ? WHERE hash = ?",
    )
    .bind(status)
    .bind(hash.get_raw_36())
    .execute(executor)
    .await?;
    Ok(result.rows_affected())
}

pub(crate) async fn set_abandoned_at<'e, E>(
    executor: E,
    hash: &DhtOpHash,
    when: Timestamp,
) -> sqlx::Result<u64>
where
    E: Executor<'e, Database = Sqlite>,
{
    let result = sqlx::query(
        "UPDATE LimboWarrant SET abandoned_at = ? WHERE hash = ?",
    )
    .bind(when.as_micros())
    .bind(hash.get_raw_36())
    .execute(executor)
    .await?;
    Ok(result.rows_affected())
}

pub(crate) async fn delete_limbo_warrant<'e, E>(executor: E, hash: DhtOpHash) -> sqlx::Result<()>
where
    E: Executor<'e, Database = Sqlite>,
{
    sqlx::query("DELETE FROM LimboWarrant WHERE hash = ?")
        .bind(hash.get_raw_36())
        .execute(executor)
        .await?;
    Ok(())
}
