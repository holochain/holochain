//! Free-standing operations against the `Warrant` and `WarrantOp` tables.
//!
//! `Warrant` holds the signed/gossiped content; `WarrantOp` holds local
//! op-level metadata (basis, integration timestamps, byte size). Together
//! they describe an integrated warrant — parallel to the `Action` /
//! `ChainOp` split for chain ops.

use crate::models::dht::WarrantRow;
use holo_hash::{AgentPubKey, DhtOpHash};
use holochain_timestamp::Timestamp;
use sqlx::{Executor, Sqlite, SqliteConnection};

/// Parameters for inserting an integrated warrant — content goes into
/// `Warrant`, op metadata into `WarrantOp`.
pub struct InsertWarrant<'a> {
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
    /// 64-byte signature over the warrant content.
    pub signature: &'a [u8],
    /// Human-readable rejection reason, denormalized out of `proof` for
    /// queryability; `None` for warrants that carry no reason.
    pub reason: Option<&'a str>,
    /// Numeric storage center derived from the warrantee.
    pub storage_center_loc: u32,
    /// Microsecond timestamp at which the warrant was received.
    pub when_received: Timestamp,
    /// Microsecond timestamp at which the warrant was integrated.
    pub when_integrated: Timestamp,
    /// Terminal sys-validation status (1 = accepted/valid, 2 = rejected).
    pub validation_status: i64,
    /// Wire-size of the warrant in bytes.
    pub serialized_size: u32,
}

/// Insert into both `Warrant` (content) and `WarrantOp` (op metadata).
///
/// The two `INSERT`s must execute atomically; the caller wraps this in a
/// transaction. Both tables declare their primary key `ON CONFLICT IGNORE`,
/// so a warrant delivered more than once (e.g. a gossip retry of an already
/// integrated warrant) is silently skipped rather than aborting the
/// transaction — no explicit `OR IGNORE` is needed.
pub(crate) async fn insert_warrant<'a>(
    conn: &mut SqliteConnection,
    w: InsertWarrant<'a>,
) -> sqlx::Result<()> {
    sqlx::query(
        "INSERT INTO Warrant (hash, author, timestamp, warrantee, proof, signature, reason)
         VALUES (?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(w.hash.get_raw_36())
    .bind(w.author.get_raw_36())
    .bind(w.timestamp.as_micros())
    .bind(w.warrantee.get_raw_36())
    .bind(w.proof)
    .bind(w.signature)
    .bind(w.reason)
    .execute(&mut *conn)
    .await?;

    sqlx::query(
        "INSERT INTO WarrantOp (hash, storage_center_loc, when_received,
                                when_integrated, validation_status, serialized_size)
         VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(w.hash.get_raw_36())
    .bind(w.storage_center_loc as i64)
    .bind(w.when_received.as_micros())
    .bind(w.when_integrated.as_micros())
    .bind(w.validation_status)
    .bind(w.serialized_size as i64)
    .execute(&mut *conn)
    .await?;

    Ok(())
}

/// Serialized `WarrantProof`s of warrants against `warrantee` that are still
/// pending validation or have been validated as **valid** — rejected and
/// abandoned warrants are excluded.
///
/// Covers both stages: limbo warrants whose sys-validation is undecided or
/// accepted (and not abandoned), and integrated warrants that were accepted.
/// Used by `is_action_warranted_as_invalid`, which decodes each proof to look
/// for an `InvalidChainOp` warrant naming a specific action.
pub(crate) async fn pending_or_valid_warrant_proofs_by_warrantee<'e, E>(
    executor: E,
    warrantee: &AgentPubKey,
) -> sqlx::Result<Vec<Vec<u8>>>
where
    E: Executor<'e, Database = Sqlite>,
{
    let rows: Vec<(Vec<u8>,)> = sqlx::query_as(
        "SELECT w.proof
         FROM Warrant w
         JOIN LimboWarrantOp op ON op.hash = w.hash
         WHERE w.warrantee = ?
           AND op.abandoned_at IS NULL
           AND (op.sys_validation_status IS NULL OR op.sys_validation_status = 1)
         UNION ALL
         SELECT w.proof
         FROM Warrant w
         JOIN WarrantOp op ON op.hash = w.hash
         WHERE w.warrantee = ?
           AND op.validation_status = 1",
    )
    .bind(warrantee.get_raw_36())
    .bind(warrantee.get_raw_36())
    .fetch_all(executor)
    .await?;
    Ok(rows.into_iter().map(|(proof,)| proof).collect())
}

pub(crate) async fn get_warrant<'e, E>(
    executor: E,
    hash: DhtOpHash,
) -> sqlx::Result<Option<WarrantRow>>
where
    E: Executor<'e, Database = Sqlite>,
{
    sqlx::query_as(
        "SELECT w.hash, w.author, w.timestamp, w.warrantee, w.proof, w.signature, w.reason,
                op.storage_center_loc, op.when_received, op.when_integrated,
                op.serialized_size
         FROM Warrant w
         JOIN WarrantOp op ON op.hash = w.hash
         WHERE w.hash = ?",
    )
    .bind(hash.get_raw_36())
    .fetch_optional(executor)
    .await
}

pub(crate) async fn get_warrants_by_warrantee<'e, E>(
    executor: E,
    warrantee: AgentPubKey,
) -> sqlx::Result<Vec<WarrantRow>>
where
    E: Executor<'e, Database = Sqlite>,
{
    sqlx::query_as(
        "SELECT w.hash, w.author, w.timestamp, w.warrantee, w.proof, w.signature, w.reason,
                op.storage_center_loc, op.when_received, op.when_integrated,
                op.serialized_size
         FROM Warrant w
         JOIN WarrantOp op ON op.hash = w.hash
         WHERE w.warrantee = ?
         ORDER BY w.timestamp DESC",
    )
    .bind(warrantee.get_raw_36())
    .fetch_all(executor)
    .await
}

/// Warrants authored by `author` (the warrant issuer), whether still in limbo
/// or integrated. The shared `Warrant` table holds the content; op metadata
/// comes from `WarrantOp` (integrated) or `LimboWarrantOp` (in validation),
/// with `when_integrated` defaulted to `0` for limbo warrants. Ordered by
/// timestamp descending.
pub(crate) async fn get_warrants_by_author<'e, E>(
    executor: E,
    author: AgentPubKey,
) -> sqlx::Result<Vec<WarrantRow>>
where
    E: Executor<'e, Database = Sqlite>,
{
    sqlx::query_as(
        "SELECT w.hash, w.author, w.timestamp, w.warrantee, w.proof, w.signature, w.reason,
                COALESCE(op.storage_center_loc, lop.storage_center_loc) AS storage_center_loc,
                COALESCE(op.when_received, lop.when_received) AS when_received,
                COALESCE(op.when_integrated, 0) AS when_integrated,
                COALESCE(op.serialized_size, lop.serialized_size) AS serialized_size
         FROM Warrant w
         LEFT JOIN WarrantOp op ON op.hash = w.hash
         LEFT JOIN LimboWarrantOp lop ON lop.hash = w.hash
         WHERE w.author = ? AND (op.hash IS NOT NULL OR lop.hash IS NOT NULL)
         ORDER BY w.timestamp DESC",
    )
    .bind(author.get_raw_36())
    .fetch_all(executor)
    .await
}

/// Validation outcome of a warrant op (`1` = valid, `2` = rejected), taken
/// from the integrated `WarrantOp` if present, otherwise from the limbo
/// warrant's `sys_validation_status` (warrants have no app-validation stage).
/// Returns `None` if no warrant op exists or its validation is still pending.
pub(crate) async fn warrant_op_validation_status<'e, E>(
    executor: E,
    op_hash: &DhtOpHash,
) -> sqlx::Result<Option<i64>>
where
    E: Executor<'e, Database = Sqlite>,
{
    let hash = op_hash.get_raw_36().to_vec();
    let (status,): (Option<i64>,) = sqlx::query_as(
        "SELECT COALESCE(
            (SELECT validation_status FROM WarrantOp WHERE hash = ?),
            (SELECT sys_validation_status FROM LimboWarrantOp WHERE hash = ?)
        )",
    )
    .bind(hash.clone())
    .bind(hash)
    .fetch_one(executor)
    .await?;
    Ok(status)
}
