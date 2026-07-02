//! Free-standing operations against the `LimboChainOp` table.

use crate::models::dht::LimboChainOpRow;
use holo_hash::{ActionHash, AnyLinkableHash, DhtOpHash};
use holochain_integrity_types::dht_v2::OpValidity;
use holochain_timestamp::Timestamp;
use sqlx::{Executor, Sqlite, SqliteConnection};

/// A row joining `LimboChainOp` with its associated `Action` and optional
/// `Entry` blob. Enables reconstructing a `DhtOpHashed` without an N+1
/// round-trip per row.
#[derive(Debug, sqlx::FromRow)]
pub struct LimboChainOpJoinedRow {
    // --- LimboChainOp columns ---
    pub hash: Vec<u8>,
    pub op_type: i64,
    pub action_hash: Vec<u8>,
    pub basis_hash: Vec<u8>,
    pub storage_center_loc: i64,
    pub sys_validation_status: Option<i64>,
    pub app_validation_status: Option<i64>,
    pub abandoned_at: Option<i64>,
    pub require_receipt: i64,
    pub when_received: i64,
    pub sys_validation_attempts: i64,
    pub app_validation_attempts: i64,
    pub last_validation_attempt: Option<i64>,
    pub serialized_size: i64,
    // --- Action columns (aliased to avoid collision) ---
    pub action_author: Vec<u8>,
    pub action_seq: i64,
    pub action_prev_hash: Option<Vec<u8>>,
    pub action_timestamp: i64,
    pub action_type: i64,
    pub action_data: Vec<u8>,
    pub action_signature: Vec<u8>,
    pub action_entry_hash: Option<Vec<u8>>,
    pub action_private_entry: Option<i64>,
    pub action_record_validity: Option<i64>,
    // --- Entry columns (LEFT JOIN — may be NULL) ---
    pub entry_blob: Option<Vec<u8>>,
}

/// Parameters for inserting a row into `LimboChainOp`.
pub struct InsertLimboChainOp<'a> {
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

/// Fetch limbo chain ops pending sys-validation, joined with their `Action`
/// and (optionally) their `Entry`, in a single query.
pub(crate) async fn limbo_chain_ops_pending_sys_validation_with_action<'e, E>(
    executor: E,
    limit: u32,
) -> sqlx::Result<Vec<LimboChainOpJoinedRow>>
where
    E: Executor<'e, Database = Sqlite>,
{
    sqlx::query_as(
        "SELECT
            l.hash              AS hash,
            l.op_type           AS op_type,
            l.action_hash       AS action_hash,
            l.basis_hash        AS basis_hash,
            l.storage_center_loc AS storage_center_loc,
            l.sys_validation_status AS sys_validation_status,
            l.app_validation_status AS app_validation_status,
            l.abandoned_at      AS abandoned_at,
            l.require_receipt   AS require_receipt,
            l.when_received     AS when_received,
            l.sys_validation_attempts AS sys_validation_attempts,
            l.app_validation_attempts AS app_validation_attempts,
            l.last_validation_attempt AS last_validation_attempt,
            l.serialized_size   AS serialized_size,
            a.author            AS action_author,
            a.seq               AS action_seq,
            a.prev_hash         AS action_prev_hash,
            a.timestamp         AS action_timestamp,
            a.action_type       AS action_type,
            a.action_data       AS action_data,
            a.signature         AS action_signature,
            a.entry_hash        AS action_entry_hash,
            a.private_entry     AS action_private_entry,
            a.record_validity   AS action_record_validity,
            e.blob              AS entry_blob
         FROM LimboChainOp l
         JOIN Action a ON l.action_hash = a.hash
         LEFT JOIN Entry e ON a.entry_hash = e.hash
         WHERE l.sys_validation_status IS NULL
         ORDER BY l.sys_validation_attempts, l.when_received
         LIMIT ?",
    )
    .bind(limit as i64)
    .fetch_all(executor)
    .await
}

/// Fetch limbo chain ops pending app-validation, joined with their `Action`
/// and (optionally) their `Entry`, in a single query.
pub(crate) async fn limbo_chain_ops_pending_app_validation_with_action<'e, E>(
    executor: E,
    limit: u32,
) -> sqlx::Result<Vec<LimboChainOpJoinedRow>>
where
    E: Executor<'e, Database = Sqlite>,
{
    sqlx::query_as(
        "SELECT
            l.hash              AS hash,
            l.op_type           AS op_type,
            l.action_hash       AS action_hash,
            l.basis_hash        AS basis_hash,
            l.storage_center_loc AS storage_center_loc,
            l.sys_validation_status AS sys_validation_status,
            l.app_validation_status AS app_validation_status,
            l.abandoned_at      AS abandoned_at,
            l.require_receipt   AS require_receipt,
            l.when_received     AS when_received,
            l.sys_validation_attempts AS sys_validation_attempts,
            l.app_validation_attempts AS app_validation_attempts,
            l.last_validation_attempt AS last_validation_attempt,
            l.serialized_size   AS serialized_size,
            a.author            AS action_author,
            a.seq               AS action_seq,
            a.prev_hash         AS action_prev_hash,
            a.timestamp         AS action_timestamp,
            a.action_type       AS action_type,
            a.action_data       AS action_data,
            a.signature         AS action_signature,
            a.entry_hash        AS action_entry_hash,
            a.private_entry     AS action_private_entry,
            a.record_validity   AS action_record_validity,
            e.blob              AS entry_blob
         FROM LimboChainOp l
         JOIN Action a ON l.action_hash = a.hash
         LEFT JOIN Entry e ON a.entry_hash = e.hash
         WHERE l.sys_validation_status = 1 AND l.app_validation_status IS NULL
         ORDER BY l.app_validation_attempts, l.when_received
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
    let inserted = sqlx::query(
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
    .await?
    .rows_affected();

    if inserted == 0 {
        // A cache-mirrored row (`locally_validated = 0`) already existed for this
        // op hash, so the INSERT was dropped by `PRIMARY KEY ON CONFLICT IGNORE`.
        // The op has now been validated locally, so upgrade the existing row.
        sqlx::query(
            "UPDATE ChainOp
             SET locally_validated = 1, validation_status = ?, when_integrated = ?
             WHERE hash = ? AND locally_validated = 0",
        )
        .bind(i64::from(validation_status))
        .bind(when_integrated.as_micros())
        .bind(&limbo.hash)
        .execute(&mut *conn)
        .await?;
    }

    // Aggregate the action's record validity from all its integrated ops (see
    // docs/design/state_model.md, "Record Validity Aggregation"): any rejected
    // op rejects the record; otherwise an accepted op accepts it. A validator
    // may hold only some of a record's ops under the sharding model, so this
    // reflects the ops known locally. Network-received actions are inserted
    // pending (`NULL`) and gain their status here on first integration.
    sqlx::query(
        "UPDATE Action
         SET record_validity = (
             SELECT CASE
                 WHEN COUNT(CASE WHEN validation_status = 2 THEN 1 END) > 0 THEN 2
                 WHEN COUNT(CASE WHEN validation_status = 1 THEN 1 END) > 0 THEN 1
                 ELSE NULL
             END
             FROM ChainOp
             WHERE action_hash = ?
         )
         WHERE hash = ?",
    )
    .bind(&limbo.action_hash)
    .bind(&limbo.action_hash)
    .execute(&mut *conn)
    .await?;

    // DELETE the limbo row.
    sqlx::query("DELETE FROM LimboChainOp WHERE hash = ?")
        .bind(op_hash.get_raw_36())
        .execute(&mut *conn)
        .await?;

    Ok(true)
}
