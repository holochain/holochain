//! Free-standing operations against the `Warrant` table.

use crate::models::dht::WarrantRow;
use holo_hash::{AgentPubKey, DhtOpHash};
use holochain_timestamp::Timestamp;
use sqlx::{Executor, Sqlite};

/// Parameters for inserting a row into `Warrant`.
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
    /// Numeric storage center derived from the warrantee.
    pub storage_center_loc: u32,
}

pub(crate) async fn insert_warrant<'a, 'e, E>(executor: E, w: InsertWarrant<'a>) -> sqlx::Result<()>
where
    E: Executor<'e, Database = Sqlite>,
{
    sqlx::query(
        "INSERT INTO Warrant (hash, author, timestamp, warrantee, proof, storage_center_loc)
         VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(w.hash.get_raw_36())
    .bind(w.author.get_raw_36())
    .bind(w.timestamp.as_micros())
    .bind(w.warrantee.get_raw_36())
    .bind(w.proof)
    .bind(w.storage_center_loc as i64)
    .execute(executor)
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
        "SELECT hash, author, timestamp, warrantee, proof, storage_center_loc
         FROM Warrant WHERE hash = ?",
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
    sqlx::query_as("SELECT * FROM Warrant WHERE warrantee = ? ORDER BY timestamp DESC")
        .bind(warrantee.get_raw_36())
        .fetch_all(executor)
        .await
}
