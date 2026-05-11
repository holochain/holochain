//! Free-standing operations against the `LimboChainOp` table.

use crate::models::dht::LimboChainOpRow;
use holo_hash::{ActionHash, AnyDhtHash, DhtOpHash};
use holochain_timestamp::Timestamp;
use sqlx::{Executor, Sqlite};

/// Parameters for inserting a row into `LimboChainOp`.
pub struct InsertLimboChainOp<'a> {
    /// DHT op hash (primary key).
    pub op_hash: &'a DhtOpHash,
    /// Hash of the action carried by this op.
    pub action_hash: &'a ActionHash,
    /// `ChainOpType` discriminant; see
    /// [`From<ChainOpType> for i64`](holochain_zome_types::dht_v2).
    pub op_type: i64,
    /// DHT basis hash (where the op is stored).
    pub basis_hash: &'a AnyDhtHash,
    /// Numeric storage center derived from `basis_hash`.
    pub storage_center_loc: u32,
    /// Whether the receiving authority should issue a validation receipt.
    pub require_receipt: bool,
    /// Microsecond timestamp at which the op was received.
    pub when_received: Timestamp,
    /// Wire-size of the op in bytes.
    pub serialized_size: u32,
}

pub(crate) async fn insert_limbo_chain_op<'a, 'e, E>(
    executor: E,
    op: InsertLimboChainOp<'a>,
) -> sqlx::Result<()>
where
    E: Executor<'e, Database = Sqlite>,
{
    sqlx::query(
        "INSERT INTO LimboChainOp
            (hash, op_type, action_hash, basis_hash, storage_center_loc,
             require_receipt, when_received, serialized_size)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(op.op_hash.get_raw_36())
    .bind(op.op_type)
    .bind(op.action_hash.get_raw_36())
    .bind(op.basis_hash.get_raw_36())
    .bind(op.storage_center_loc as i64)
    .bind(op.require_receipt as i64)
    .bind(op.when_received.as_micros())
    .bind(op.serialized_size as i64)
    .execute(executor)
    .await?;
    Ok(())
}

pub(crate) async fn get_limbo_chain_op<'e, E>(
    executor: E,
    hash: DhtOpHash,
) -> sqlx::Result<Option<LimboChainOpRow>>
where
    E: Executor<'e, Database = Sqlite>,
{
    sqlx::query_as(
        "SELECT hash, op_type, action_hash, basis_hash, storage_center_loc,
                sys_validation_status, app_validation_status, abandoned_at,
                require_receipt, when_received, sys_validation_attempts,
                app_validation_attempts, last_validation_attempt, serialized_size
         FROM LimboChainOp WHERE hash = ?",
    )
    .bind(hash.get_raw_36())
    .fetch_optional(executor)
    .await
}

pub(crate) async fn limbo_chain_ops_pending_sys_validation<'e, E>(
    executor: E,
    limit: u32,
) -> sqlx::Result<Vec<LimboChainOpRow>>
where
    E: Executor<'e, Database = Sqlite>,
{
    sqlx::query_as(
        "SELECT * FROM LimboChainOp
         WHERE sys_validation_status IS NULL AND abandoned_at IS NULL
         ORDER BY sys_validation_attempts, when_received
         LIMIT ?",
    )
    .bind(limit as i64)
    .fetch_all(executor)
    .await
}

pub(crate) async fn limbo_chain_ops_pending_app_validation<'e, E>(
    executor: E,
    limit: u32,
) -> sqlx::Result<Vec<LimboChainOpRow>>
where
    E: Executor<'e, Database = Sqlite>,
{
    sqlx::query_as(
        "SELECT * FROM LimboChainOp
         WHERE sys_validation_status = 1 AND app_validation_status IS NULL
           AND abandoned_at IS NULL
         ORDER BY app_validation_attempts, when_received
         LIMIT ?",
    )
    .bind(limit as i64)
    .fetch_all(executor)
    .await
}

pub(crate) async fn limbo_chain_ops_ready_for_integration<'e, E>(
    executor: E,
    limit: u32,
) -> sqlx::Result<Vec<LimboChainOpRow>>
where
    E: Executor<'e, Database = Sqlite>,
{
    sqlx::query_as(
        "SELECT * FROM LimboChainOp
         WHERE abandoned_at IS NOT NULL
            OR sys_validation_status = 2
            OR (sys_validation_status = 1 AND app_validation_status IN (1, 2))
         ORDER BY when_received
         LIMIT ?",
    )
    .bind(limit as i64)
    .fetch_all(executor)
    .await
}

pub(crate) async fn set_sys_validation_status<'e, E>(
    executor: E,
    op_hash: &DhtOpHash,
    status: Option<i64>,
) -> sqlx::Result<u64>
where
    E: Executor<'e, Database = Sqlite>,
{
    let result = sqlx::query(
        "UPDATE LimboChainOp SET sys_validation_status = ? WHERE hash = ?",
    )
    .bind(status)
    .bind(op_hash.get_raw_36())
    .execute(executor)
    .await?;
    Ok(result.rows_affected())
}

pub(crate) async fn set_app_validation_status<'e, E>(
    executor: E,
    op_hash: &DhtOpHash,
    status: Option<i64>,
) -> sqlx::Result<u64>
where
    E: Executor<'e, Database = Sqlite>,
{
    let result = sqlx::query(
        "UPDATE LimboChainOp SET app_validation_status = ? WHERE hash = ?",
    )
    .bind(status)
    .bind(op_hash.get_raw_36())
    .execute(executor)
    .await?;
    Ok(result.rows_affected())
}

pub(crate) async fn set_abandoned_at<'e, E>(
    executor: E,
    op_hash: &DhtOpHash,
    when: Timestamp,
) -> sqlx::Result<u64>
where
    E: Executor<'e, Database = Sqlite>,
{
    let result = sqlx::query(
        "UPDATE LimboChainOp SET abandoned_at = ? WHERE hash = ?",
    )
    .bind(when.as_micros())
    .bind(op_hash.get_raw_36())
    .execute(executor)
    .await?;
    Ok(result.rows_affected())
}

pub(crate) async fn delete_limbo_chain_op<'e, E>(executor: E, hash: DhtOpHash) -> sqlx::Result<()>
where
    E: Executor<'e, Database = Sqlite>,
{
    sqlx::query("DELETE FROM LimboChainOp WHERE hash = ?")
        .bind(hash.get_raw_36())
        .execute(executor)
        .await?;
    Ok(())
}
