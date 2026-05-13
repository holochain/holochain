//! Free-standing operations against the `ChainOp` table.

use crate::models::dht::ChainOpRow;
use holo_hash::{ActionHash, AnyDhtHash, DhtOpHash};
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
    /// DHT basis hash (where the op is stored).
    pub basis_hash: &'a AnyDhtHash,
    /// Numeric storage center derived from `basis_hash`.
    pub storage_center_loc: u32,
    /// Final validation outcome.
    pub validation_status: OpValidity,
    /// `true` when this authority locally validated the op; `false` when
    /// accepted via receipts only.
    pub locally_validated: bool,
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
             validation_status, locally_validated, when_received, when_integrated,
             serialized_size)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(op.op_hash.get_raw_36())
    .bind(op.op_type)
    .bind(op.action_hash.get_raw_36())
    .bind(op.basis_hash.get_raw_36())
    .bind(op.storage_center_loc as i64)
    .bind(i64::from(op.validation_status))
    .bind(op.locally_validated as i64)
    .bind(op.when_received.as_micros())
    .bind(op.when_integrated.as_micros())
    .bind(op.serialized_size as i64)
    .execute(executor)
    .await?;
    Ok(())
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
                validation_status, locally_validated, when_received, when_integrated,
                serialized_size
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
