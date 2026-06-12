//! Free-standing operations against the `ChainOp` table.

use crate::models::dht::ChainOpRow;
use holo_hash::{ActionHash, AnyDhtHash, AnyLinkableHash, DhtOpHash};
use holochain_integrity_types::dht_v2::OpValidity;
use holochain_timestamp::Timestamp;
use sqlx::{Executor, Sqlite};

/// Parameters for inserting a row into `ChainOp`.
pub struct InsertChainOp<'a> {
    /// DHT op hash (primary key).
    pub op_hash: &'a DhtOpHash,
    /// Hash of the action carried by this op.
    pub action_hash: &'a ActionHash,
    /// `ChainOpType` discriminant; see
    /// [`From<ChainOpType> for i64`](holochain_zome_types::dht_v2).
    pub op_type: i64,
    /// DHT basis hash (`OpBasis`) where the op is stored.
    /// `AnyLinkableHash`, not `AnyDhtHash`: link-op bases may be `External`
    /// hashes, which `AnyDhtHash` cannot hold.
    pub basis_hash: &'a AnyLinkableHash,
    /// Numeric storage center derived from `basis_hash`.
    pub storage_center_loc: u32,
    /// Final validation outcome.
    pub validation_status: OpValidity,
    /// `true` when this authority locally validated the op; `false` when
    /// accepted via receipts only.
    pub locally_validated: bool,
    /// `true` while a validation receipt is still owed to the op's author.
    pub require_receipt: bool,
    /// Microsecond timestamp at which the op was received.
    pub when_received: Timestamp,
    /// Microsecond timestamp at which the op was integrated.
    pub when_integrated: Timestamp,
    /// Wire-size of the op in bytes.
    pub serialized_size: u32,
}

pub(crate) async fn insert_chain_op<'a, 'e, E>(
    executor: E,
    op: InsertChainOp<'a>,
) -> sqlx::Result<()>
where
    E: Executor<'e, Database = Sqlite>,
{
    sqlx::query(
        "INSERT INTO ChainOp
            (hash, op_type, action_hash, basis_hash, storage_center_loc,
             validation_status, locally_validated, require_receipt,
             when_received, when_integrated, serialized_size)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(op.op_hash.get_raw_36())
    .bind(op.op_type)
    .bind(op.action_hash.get_raw_36())
    .bind(op.basis_hash.get_raw_36())
    .bind(op.storage_center_loc as i64)
    .bind(i64::from(op.validation_status))
    .bind(op.locally_validated as i64)
    .bind(op.require_receipt as i64)
    .bind(op.when_received.as_micros())
    .bind(op.when_integrated.as_micros())
    .bind(op.serialized_size as i64)
    .execute(executor)
    .await?;
    Ok(())
}

/// Clear the `require_receipt` flag for the given op on the `ChainOp` table.
/// Returns the number of rows updated.
pub(crate) async fn clear_require_receipt<'e, E>(
    executor: E,
    op_hash: &DhtOpHash,
) -> sqlx::Result<u64>
where
    E: Executor<'e, Database = Sqlite>,
{
    let result = sqlx::query("UPDATE ChainOp SET require_receipt = 0 WHERE hash = ?")
        .bind(op_hash.get_raw_36())
        .execute(executor)
        .await?;
    Ok(result.rows_affected())
}

pub(crate) async fn get_chain_op<'e, E>(
    executor: E,
    hash: DhtOpHash,
) -> sqlx::Result<Option<ChainOpRow>>
where
    E: Executor<'e, Database = Sqlite>,
{
    sqlx::query_as(
        "SELECT hash, op_type, action_hash, basis_hash, storage_center_loc,
                validation_status, locally_validated, require_receipt,
                when_received, when_integrated, serialized_size
         FROM ChainOp WHERE hash = ?",
    )
    .bind(hash.get_raw_36())
    .fetch_optional(executor)
    .await
}

pub(crate) async fn get_chain_ops_by_basis<'e, E>(
    executor: E,
    basis: AnyDhtHash,
) -> sqlx::Result<Vec<ChainOpRow>>
where
    E: Executor<'e, Database = Sqlite>,
{
    sqlx::query_as("SELECT * FROM ChainOp WHERE basis_hash = ? ORDER BY when_integrated")
        .bind(basis.get_raw_36())
        .fetch_all(executor)
        .await
}

/// Update `validation_status` to `Rejected` for the given op only when the
/// current status is `Accepted` and the op is not locally validated.  Returns
/// the number of rows updated.
///
/// This enforces a one-way `Accepted → Rejected` transition on network-cached
/// ops only.  Locally-authored or locally-validated ops never change status
/// through this path — a status change for such ops would require a warrant
/// and reprocessing.
pub(crate) async fn set_validation_status<'e, E>(
    executor: E,
    op_hash: &DhtOpHash,
    validation_status: OpValidity,
) -> sqlx::Result<u64>
where
    E: Executor<'e, Database = Sqlite>,
{
    let accepted = i64::from(OpValidity::Accepted);
    let result = sqlx::query(
        "UPDATE ChainOp SET validation_status = ?
         WHERE hash = ? AND validation_status = ? AND locally_validated = 0",
    )
    .bind(i64::from(validation_status))
    .bind(op_hash.get_raw_36())
    .bind(accepted)
    .execute(executor)
    .await?;
    Ok(result.rows_affected())
}

pub(crate) async fn get_chain_ops_for_action<'e, E>(
    executor: E,
    action_hash: ActionHash,
) -> sqlx::Result<Vec<ChainOpRow>>
where
    E: Executor<'e, Database = Sqlite>,
{
    sqlx::query_as("SELECT * FROM ChainOp WHERE action_hash = ? ORDER BY op_type")
        .bind(action_hash.get_raw_36())
        .fetch_all(executor)
        .await
}

/// Terminal validation outcome (`1` = Valid/Accepted, `2` = Rejected) of the
/// chain op for `(action_hash, op_type)`, taken from whichever of the
/// integrated `ChainOp` (preferred) or the in-validation `LimboChainOp` holds a
/// decided result. Returns `None` when the op is still pending validation,
/// cache-only, or absent. Used by the warrant-dependency readiness check.
///
/// The `LimboChainOp` branch matches the legacy `get_dht_op_validation_state`,
/// which surfaced a validation decision *before* the op was integrated: a
/// dependency that has been validated (sys-rejected, or sys-accepted with an
/// app decision) is ready even though the integration workflow has not yet
/// promoted it.
pub(crate) async fn op_validation_outcome<'e, E>(
    executor: E,
    action_hash: &ActionHash,
    op_type: i64,
) -> sqlx::Result<Option<i64>>
where
    E: Executor<'e, Database = Sqlite>,
{
    sqlx::query_scalar(
        "SELECT outcome FROM (
            SELECT validation_status AS outcome, 0 AS pri
            FROM ChainOp
            WHERE action_hash = ? AND op_type = ? AND locally_validated = 1
            UNION ALL
            SELECT
                (CASE WHEN sys_validation_status = 2 OR app_validation_status = 2
                      THEN 2 ELSE 1 END) AS outcome,
                1 AS pri
            FROM LimboChainOp
            WHERE action_hash = ? AND op_type = ?
              AND (sys_validation_status = 2
                   OR (sys_validation_status = 1 AND app_validation_status IN (1, 2)))
         )
         ORDER BY pri
         LIMIT 1",
    )
    .bind(action_hash.get_raw_36())
    .bind(op_type)
    .bind(action_hash.get_raw_36())
    .bind(op_type)
    .fetch_optional(executor)
    .await
}

/// Row returned by [`pending_validation_receipts`]: op metadata plus the
/// underlying action's author so receipts can be addressed.
#[derive(Debug, sqlx::FromRow)]
pub struct PendingReceiptRow {
    /// Raw 36-byte op hash from `ChainOp.hash`.
    pub op_hash: Vec<u8>,
    /// Validation status integer (`1` = Accepted, `2` = Rejected).
    pub validation_status: i64,
    /// Microsecond timestamp at which the op was integrated.
    pub when_integrated: i64,
    /// Raw 36-byte author public key from `Action.author`.
    pub action_author: Vec<u8>,
}

/// Return integrated, validated [`ChainOp`] rows that still require a
/// validation receipt to be sent to the action author.
pub(crate) async fn pending_validation_receipts<'e, E>(
    executor: E,
) -> sqlx::Result<Vec<PendingReceiptRow>>
where
    E: Executor<'e, Database = Sqlite>,
{
    sqlx::query_as(
        "SELECT
            ChainOp.hash AS op_hash,
            ChainOp.validation_status AS validation_status,
            ChainOp.when_integrated AS when_integrated,
            Action.author AS action_author
         FROM ChainOp
         JOIN Action ON ChainOp.action_hash = Action.hash
         WHERE ChainOp.require_receipt = 1
           AND ChainOp.when_integrated IS NOT NULL
           AND ChainOp.validation_status IS NOT NULL",
    )
    .fetch_all(executor)
    .await
}
