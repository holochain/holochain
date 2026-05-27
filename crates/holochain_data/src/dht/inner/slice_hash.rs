//! Free-standing operations against the `SliceHash` table.
//!
//! K2's gossip layer hashes contiguous arc slices and stores the resulting
//! hash per `(arc_start, arc_end, slice_index)`. Re-storing the same slice
//! replaces the prior hash (the table's PK has `ON CONFLICT REPLACE`).

use crate::models::dht::SliceHashIndexedRow;
use sqlx::{Executor, Sqlite};

/// Insert or replace the slice hash for `(arc_start, arc_end, slice_index)`.
pub(crate) async fn insert_slice_hash<'e, E>(
    executor: E,
    arc_start: u32,
    arc_end: u32,
    slice_index: u64,
    hash: &[u8],
) -> sqlx::Result<()>
where
    E: Executor<'e, Database = Sqlite>,
{
    sqlx::query(
        "INSERT INTO SliceHash (arc_start, arc_end, slice_index, hash)
         VALUES (?, ?, ?, ?)",
    )
    .bind(arc_start as i64)
    .bind(arc_end as i64)
    .bind(slice_index as i64)
    .bind(hash)
    .execute(executor)
    .await?;
    Ok(())
}

/// Number of stored slices for the arc, or 0 if none.
///
/// K2 assigns slice indices consecutively from 0, so the count is the
/// highest stored index + 1. This matches the kitsune2 reference op-store,
/// which returns `highest_stored_id + 1`. A plain `MAX(slice_index)` would
/// undercount by one and could not tell "no slices" apart from "one slice
/// at index 0", so read the nullable `MAX` and add one only when a row
/// exists.
pub(crate) async fn slice_hash_count<'e, E>(
    executor: E,
    arc_start: u32,
    arc_end: u32,
) -> sqlx::Result<u64>
where
    E: Executor<'e, Database = Sqlite>,
{
    let (max_index,): (Option<i64>,) = sqlx::query_as(
        "SELECT MAX(slice_index) FROM SliceHash
         WHERE arc_start = ? AND arc_end = ?",
    )
    .bind(arc_start as i64)
    .bind(arc_end as i64)
    .fetch_one(executor)
    .await?;
    Ok(max_index.map_or(0, |m| m.max(0) as u64 + 1))
}

/// Fetch a single stored slice hash, if any.
pub(crate) async fn get_slice_hash<'e, E>(
    executor: E,
    arc_start: u32,
    arc_end: u32,
    slice_index: u64,
) -> sqlx::Result<Option<Vec<u8>>>
where
    E: Executor<'e, Database = Sqlite>,
{
    let row: Option<(Vec<u8>,)> = sqlx::query_as(
        "SELECT hash FROM SliceHash
         WHERE arc_start = ? AND arc_end = ? AND slice_index = ?",
    )
    .bind(arc_start as i64)
    .bind(arc_end as i64)
    .bind(slice_index as i64)
    .fetch_optional(executor)
    .await?;
    Ok(row.map(|(h,)| h))
}

/// Fetch every `(slice_index, hash)` pair stored for the arc, in no
/// particular order. K2's callers don't rely on ordering here.
pub(crate) async fn get_slice_hashes<'e, E>(
    executor: E,
    arc_start: u32,
    arc_end: u32,
) -> sqlx::Result<Vec<SliceHashIndexedRow>>
where
    E: Executor<'e, Database = Sqlite>,
{
    sqlx::query_as::<_, SliceHashIndexedRow>(
        "SELECT slice_index, hash FROM SliceHash
         WHERE arc_start = ? AND arc_end = ?",
    )
    .bind(arc_start as i64)
    .bind(arc_end as i64)
    .fetch_all(executor)
    .await
}
