//! Free-standing operations against the `LimboWarrant` table.

use crate::models::dht::LimboWarrantRow;
use holo_hash::{AgentPubKey, DhtOpHash};
use holochain_timestamp::Timestamp;
use sqlx::{Executor, Sqlite, SqliteConnection};

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
         WHERE sys_validation_status IS NULL
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
         WHERE sys_validation_status IN (1, 2)
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
        "UPDATE LimboWarrant SET sys_validation_status = ?
         WHERE hash = ? AND sys_validation_status IS NULL",
    )
    .bind(status)
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

/// Promote a `LimboWarrant` row to the `Warrant` table.
///
/// Reads the limbo row, inserts into `Warrant` with the carried fields, then
/// deletes the limbo row. All three statements run on the given connection.
/// **The caller must wrap this call in a transaction** to ensure atomicity.
///
/// Returns `true` if the limbo row existed and was promoted, `false` if it
/// did not exist.
///
/// Note: the `Warrant` table has no `when_integrated` column, so no
/// integration timestamp is stored.
pub(crate) async fn promote_to_warrant(
    conn: &mut SqliteConnection,
    hash: &DhtOpHash,
) -> sqlx::Result<bool> {
    // SELECT the limbo row's payload.
    let row: Option<LimboWarrantRow> = sqlx::query_as(
        "SELECT hash, author, timestamp, warrantee, proof, storage_center_loc,
                sys_validation_status, abandoned_at, when_received,
                sys_validation_attempts, last_validation_attempt, serialized_size
         FROM LimboWarrant WHERE hash = ?",
    )
    .bind(hash.get_raw_36())
    .fetch_optional(&mut *conn)
    .await?;

    let limbo = match row {
        Some(r) => r,
        None => return Ok(false),
    };

    // INSERT into Warrant.
    sqlx::query(
        "INSERT INTO Warrant (hash, author, timestamp, warrantee, proof, storage_center_loc)
         VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(&limbo.hash)
    .bind(&limbo.author)
    .bind(limbo.timestamp)
    .bind(&limbo.warrantee)
    .bind(&limbo.proof)
    .bind(limbo.storage_center_loc)
    .execute(&mut *conn)
    .await?;

    // DELETE the limbo row.
    sqlx::query("DELETE FROM LimboWarrant WHERE hash = ?")
        .bind(hash.get_raw_36())
        .execute(&mut *conn)
        .await?;

    Ok(true)
}
