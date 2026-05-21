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
    /// Numeric storage center derived from the warrantee.
    pub storage_center_loc: u32,
    /// Microsecond timestamp at which the warrant was received.
    pub when_received: Timestamp,
    /// Microsecond timestamp at which the warrant was integrated.
    pub when_integrated: Timestamp,
    /// Wire-size of the warrant in bytes.
    pub serialized_size: u32,
}

/// Insert into both `Warrant` (content) and `WarrantOp` (op metadata).
///
/// The two `INSERT`s must execute atomically; the caller is responsible
/// for wrapping this call in a transaction. `INSERT INTO Warrant ... ON
/// CONFLICT IGNORE` lets the content row already exist (e.g. when promoting
/// from limbo where it was inserted alongside `LimboWarrantOp`).
pub(crate) async fn insert_warrant<'a>(
    conn: &mut SqliteConnection,
    w: InsertWarrant<'a>,
) -> sqlx::Result<()> {
    sqlx::query(
        "INSERT INTO Warrant (hash, author, timestamp, warrantee, proof, signature)
         VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(w.hash.get_raw_36())
    .bind(w.author.get_raw_36())
    .bind(w.timestamp.as_micros())
    .bind(w.warrantee.get_raw_36())
    .bind(w.proof)
    .bind(w.signature)
    .execute(&mut *conn)
    .await?;

    sqlx::query(
        "INSERT INTO WarrantOp (hash, storage_center_loc, when_received,
                                when_integrated, serialized_size)
         VALUES (?, ?, ?, ?, ?)",
    )
    .bind(w.hash.get_raw_36())
    .bind(w.storage_center_loc as i64)
    .bind(w.when_received.as_micros())
    .bind(w.when_integrated.as_micros())
    .bind(w.serialized_size as i64)
    .execute(&mut *conn)
    .await?;

    Ok(())
}

pub(crate) async fn get_warrant<'e, E>(
    executor: E,
    hash: DhtOpHash,
) -> sqlx::Result<Option<WarrantRow>>
where
    E: Executor<'e, Database = Sqlite>,
{
    sqlx::query_as(
        "SELECT w.hash, w.author, w.timestamp, w.warrantee, w.proof, w.signature,
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
        "SELECT w.hash, w.author, w.timestamp, w.warrantee, w.proof, w.signature,
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
