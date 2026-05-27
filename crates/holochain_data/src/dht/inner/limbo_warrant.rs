//! Free-standing operations against the `Warrant` and `LimboWarrantOp` tables.
//!
//! `Warrant` content is inserted alongside `LimboWarrantOp` when a warrant
//! enters limbo, and stays in place when the op is promoted to `WarrantOp`
//! at integration time — only the metadata table changes.

use crate::models::dht::LimboWarrantRow;
use holo_hash::{AgentPubKey, DhtOpHash};
use holochain_timestamp::Timestamp;
use sqlx::{Executor, Sqlite, SqliteConnection};

/// Parameters for inserting a warrant into limbo — content goes into
/// `Warrant`, op metadata into `LimboWarrantOp`.
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
    /// 64-byte signature over the warrant content.
    pub signature: &'a [u8],
    /// Human-readable rejection reason, denormalized out of `proof` for
    /// queryability; `None` for warrants that carry no reason.
    pub reason: Option<&'a str>,
    /// Numeric storage center derived from the warrantee.
    pub storage_center_loc: u32,
    /// Microsecond timestamp at which the warrant was received.
    pub when_received: Timestamp,
    /// Wire-size of the warrant in bytes.
    pub serialized_size: u32,
}

/// Insert into `Warrant` (content) and `LimboWarrantOp` (op metadata).
///
/// The two `INSERT`s must execute atomically; the caller is responsible
/// for wrapping this call in a transaction.
pub(crate) async fn insert_limbo_warrant<'a>(
    conn: &mut SqliteConnection,
    w: InsertLimboWarrant<'a>,
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
        "INSERT INTO LimboWarrantOp
            (hash, storage_center_loc, when_received, serialized_size)
         VALUES (?, ?, ?, ?)",
    )
    .bind(w.hash.get_raw_36())
    .bind(w.storage_center_loc as i64)
    .bind(w.when_received.as_micros())
    .bind(w.serialized_size as i64)
    .execute(&mut *conn)
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
        "SELECT w.hash, w.author, w.timestamp, w.warrantee, w.proof, w.signature, w.reason,
                op.storage_center_loc, op.sys_validation_status, op.abandoned_at,
                op.when_received, op.sys_validation_attempts, op.last_validation_attempt,
                op.serialized_size
         FROM Warrant w
         JOIN LimboWarrantOp op ON op.hash = w.hash
         WHERE w.hash = ?",
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
        "SELECT w.hash, w.author, w.timestamp, w.warrantee, w.proof, w.signature, w.reason,
                op.storage_center_loc, op.sys_validation_status, op.abandoned_at,
                op.when_received, op.sys_validation_attempts, op.last_validation_attempt,
                op.serialized_size
         FROM Warrant w
         JOIN LimboWarrantOp op ON op.hash = w.hash
         WHERE op.sys_validation_status IS NULL
         ORDER BY op.sys_validation_attempts, op.when_received
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
        "SELECT w.hash, w.author, w.timestamp, w.warrantee, w.proof, w.signature, w.reason,
                op.storage_center_loc, op.sys_validation_status, op.abandoned_at,
                op.when_received, op.sys_validation_attempts, op.last_validation_attempt,
                op.serialized_size
         FROM Warrant w
         JOIN LimboWarrantOp op ON op.hash = w.hash
         WHERE op.sys_validation_status IN (1, 2)
         ORDER BY op.when_received
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
        "UPDATE LimboWarrantOp SET sys_validation_status = ?
         WHERE hash = ? AND sys_validation_status IS NULL",
    )
    .bind(status)
    .bind(hash.get_raw_36())
    .execute(executor)
    .await?;
    Ok(result.rows_affected())
}

/// Delete a warrant in limbo: removes both the `LimboWarrantOp` row and the
/// underlying `Warrant` content (the content was inserted alongside the
/// limbo row and isn't useful without it).
///
/// **The caller must wrap this call in a transaction** to keep the two
/// deletes atomic.
pub(crate) async fn delete_limbo_warrant(
    conn: &mut SqliteConnection,
    hash: DhtOpHash,
) -> sqlx::Result<()> {
    sqlx::query("DELETE FROM LimboWarrantOp WHERE hash = ?")
        .bind(hash.get_raw_36())
        .execute(&mut *conn)
        .await?;
    sqlx::query("DELETE FROM Warrant WHERE hash = ?")
        .bind(hash.get_raw_36())
        .execute(&mut *conn)
        .await?;
    Ok(())
}

/// Promote a warrant from limbo to integrated: copies the `LimboWarrantOp`
/// row into `WarrantOp` (stamping `when_integrated`) and deletes the limbo
/// row. The shared `Warrant` content row is left in place.
///
/// **The caller must wrap this call in a transaction** to ensure atomicity.
///
/// Returns `true` if the limbo row existed and was promoted, `false` if it
/// did not exist.
pub(crate) async fn promote_to_warrant(
    conn: &mut SqliteConnection,
    hash: &DhtOpHash,
    when_integrated: Timestamp,
) -> sqlx::Result<bool> {
    let result = sqlx::query(
        "INSERT INTO WarrantOp (hash, storage_center_loc, when_received,
                                when_integrated, serialized_size)
         SELECT hash, storage_center_loc, when_received, ?, serialized_size
         FROM LimboWarrantOp WHERE hash = ?",
    )
    .bind(when_integrated.as_micros())
    .bind(hash.get_raw_36())
    .execute(&mut *conn)
    .await?;

    if result.rows_affected() == 0 {
        return Ok(false);
    }

    sqlx::query("DELETE FROM LimboWarrantOp WHERE hash = ?")
        .bind(hash.get_raw_36())
        .execute(&mut *conn)
        .await?;

    Ok(true)
}
