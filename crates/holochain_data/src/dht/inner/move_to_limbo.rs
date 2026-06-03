//! Move a `ChainOp` row (cached, `locally_validated = 0`) back into
//! `LimboChainOp` with cleared validation status.

use holo_hash::ActionHash;
use sqlx::SqliteConnection;

/// Columns read from `ChainOp` when moving a cached row to `LimboChainOp`:
/// `(hash, basis_hash, storage_center_loc, when_received, serialized_size)`.
type CachedChainOpRow = (Vec<u8>, Vec<u8>, i64, i64, i64);

/// Move a `ChainOp` row with `locally_validated = 0` matching
/// `(action_hash, op_type)` back into `LimboChainOp` with cleared
/// validation status. Used by sys-validation to re-validate
/// warrant-dependency ops that were originally inserted via the cache
/// path (which bypasses limbo).
///
/// Returns `Ok(true)` if a row was moved, `Ok(false)` if no matching
/// cached row exists. **Caller must wrap in a transaction** to ensure
/// atomicity.
pub(crate) async fn move_chain_op_to_limbo(
    conn: &mut SqliteConnection,
    action_hash: &ActionHash,
    op_type: i64,
) -> sqlx::Result<bool> {
    // Look up the ChainOp row we want to move.
    let row: Option<CachedChainOpRow> = sqlx::query_as(
        "SELECT hash, basis_hash, storage_center_loc, when_received, serialized_size
         FROM ChainOp
         WHERE action_hash = ?1 AND op_type = ?2 AND locally_validated = 0",
    )
    .bind(action_hash.get_raw_36())
    .bind(op_type)
    .fetch_optional(&mut *conn)
    .await?;

    let Some((op_hash, basis_hash, storage_center_loc, when_received, serialized_size)) = row
    else {
        return Ok(false);
    };

    // Insert a LimboChainOp row with cleared validation status, then delete
    // the ChainOp row. Creating the new state before destroying the old keeps
    // the intent clear; both statements run in the caller's transaction.
    sqlx::query(
        "INSERT INTO LimboChainOp
            (hash, op_type, action_hash, basis_hash, storage_center_loc,
             require_receipt, when_received, serialized_size,
             sys_validation_status, app_validation_status, abandoned_at,
             sys_validation_attempts, app_validation_attempts, last_validation_attempt)
         VALUES (?, ?, ?, ?, ?, 0, ?, ?, NULL, NULL, NULL, 0, 0, NULL)",
    )
    .bind(&op_hash)
    .bind(op_type)
    .bind(action_hash.get_raw_36())
    .bind(&basis_hash)
    .bind(storage_center_loc)
    .bind(when_received)
    .bind(serialized_size)
    .execute(&mut *conn)
    .await?;

    // Delete the ChainOp row.
    sqlx::query("DELETE FROM ChainOp WHERE hash = ?")
        .bind(&op_hash)
        .execute(&mut *conn)
        .await?;

    Ok(true)
}
