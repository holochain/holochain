//! Free-standing operations against the `LimboChainOp` table.

use crate::models::dht::LimboChainOpRow;
use holo_hash::{ActionHash, AnyDhtHash, DhtOpHash};
use holochain_integrity_types::dht_v2::OpValidity;
use holochain_timestamp::Timestamp;
use sqlx::{Executor, Sqlite, SqliteConnection};

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
         WHERE sys_validation_status IS NULL
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
         WHERE sys_validation_status = 2
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
        "UPDATE LimboChainOp SET sys_validation_status = ?
         WHERE hash = ? AND sys_validation_status IS NULL",
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
        "UPDATE LimboChainOp SET app_validation_status = ?
         WHERE hash = ? AND sys_validation_status IS NOT NULL AND app_validation_status IS NULL",
    )
    .bind(status)
    .bind(op_hash.get_raw_36())
    .execute(executor)
    .await?;
    Ok(result.rows_affected())
}

/// Force a limbo op into a rejected terminal state, bypassing the normal
/// validation ordering.  Used by `reject_chain_ops` for ops that fail outside
/// the normal sys/app validation workflows.
///
/// The rejection is recorded at whichever stage has not yet completed:
///
/// - `sys_validation_status IS NULL` → set `sys_validation_status = Rejected`,
///   leave `app_validation_status` as NULL.
/// - `sys_validation_status = Accepted, app_validation_status IS NULL` →
///   set `app_validation_status = Rejected`.
///
/// Returns the number of rows updated (0 or 1).  Rows already at a terminal
/// state (`sys = Rejected`, or `app IN (Accepted, Rejected)`) are not modified.
pub(crate) async fn force_reject<'e, E>(executor: E, op_hash: &DhtOpHash) -> sqlx::Result<u64>
where
    E: Executor<'e, Database = Sqlite>,
{
    let result = sqlx::query(
        "UPDATE LimboChainOp SET
             sys_validation_status = CASE
                 WHEN sys_validation_status IS NULL THEN 2
                 ELSE sys_validation_status
             END,
             app_validation_status = CASE
                 WHEN sys_validation_status = 1 AND app_validation_status IS NULL THEN 2
                 ELSE app_validation_status
             END
         WHERE hash = ?
           AND (sys_validation_status IS NULL
                OR (sys_validation_status = 1 AND app_validation_status IS NULL))",
    )
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

/// Promote a `LimboChainOp` row to the `ChainOp` table.
///
/// Reads the limbo row, inserts into `ChainOp` with the supplied
/// `validation_status` and `when_integrated`, then deletes the limbo row.
/// All three statements run on the given connection.
/// **The caller must wrap this call in a transaction** to ensure atomicity.
///
/// Returns `true` if the limbo row existed and was promoted, `false` if it
/// did not exist.
pub(crate) async fn promote_to_chain_op(
    conn: &mut SqliteConnection,
    op_hash: &DhtOpHash,
    validation_status: OpValidity,
    when_integrated: Timestamp,
) -> sqlx::Result<bool> {
    // SELECT the limbo row's payload.
    let row: Option<LimboChainOpRow> = sqlx::query_as(
        "SELECT hash, op_type, action_hash, basis_hash, storage_center_loc,
                sys_validation_status, app_validation_status, abandoned_at,
                require_receipt, when_received, sys_validation_attempts,
                app_validation_attempts, last_validation_attempt, serialized_size
         FROM LimboChainOp WHERE hash = ?",
    )
    .bind(op_hash.get_raw_36())
    .fetch_optional(&mut *conn)
    .await?;

    let limbo = match row {
        Some(r) => r,
        None => return Ok(false),
    };

    // INSERT into ChainOp. Carry `require_receipt` over so the receipt
    // workflow can find the op on `ChainOp` and clear the flag once the
    // receipt has been sent.
    sqlx::query(
        "INSERT INTO ChainOp
            (hash, op_type, action_hash, basis_hash, storage_center_loc,
             validation_status, locally_validated, require_receipt,
             when_received, when_integrated, serialized_size)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&limbo.hash)
    .bind(limbo.op_type)
    .bind(&limbo.action_hash)
    .bind(&limbo.basis_hash)
    .bind(limbo.storage_center_loc)
    .bind(i64::from(validation_status))
    .bind(1_i64) // locally_validated = true: op has been through limbo and validated locally
    .bind(limbo.require_receipt)
    .bind(limbo.when_received)
    .bind(when_integrated.as_micros())
    .bind(limbo.serialized_size)
    .execute(&mut *conn)
    .await?;

    // DELETE the limbo row.
    sqlx::query("DELETE FROM LimboChainOp WHERE hash = ?")
        .bind(op_hash.get_raw_36())
        .execute(&mut *conn)
        .await?;

    Ok(true)
}
