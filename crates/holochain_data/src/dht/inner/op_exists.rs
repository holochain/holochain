//! Existence checks for op hashes across all op-bearing DHT tables.

use holo_hash::DhtOpHash;
use sqlx::{Executor, Sqlite};

/// Returns `true` if the given op hash appears in any of the op-bearing
/// tables (`ChainOp`, `LimboChainOp`, `WarrantOp`, `LimboWarrantOp`).
///
/// Used by the incoming-ops workflow to filter ops that have already
/// been recorded locally so we don't re-process duplicates from the
/// network.
pub(crate) async fn op_exists<'e, E>(executor: E, hash: &DhtOpHash) -> sqlx::Result<bool>
where
    E: Executor<'e, Database = Sqlite>,
{
    let bytes = hash.get_raw_36();
    let row: (i64,) = sqlx::query_as(
        "SELECT EXISTS (
            SELECT 1 FROM ChainOp WHERE hash = ?1
            UNION ALL
            SELECT 1 FROM LimboChainOp WHERE hash = ?1
            UNION ALL
            SELECT 1 FROM WarrantOp WHERE hash = ?1
            UNION ALL
            SELECT 1 FROM LimboWarrantOp WHERE hash = ?1
            LIMIT 1
         )",
    )
    .bind(bytes)
    .fetch_one(executor)
    .await?;
    Ok(row.0 != 0)
}

/// For each input hash, return whether it appears in any op-bearing
/// table. Result vector aligns 1:1 with the input.
///
/// Performs N round-trips for simplicity. If profiling shows it
/// matters, switch to a single `WHERE ... IN (?, ?, ...)` query later.
pub(crate) async fn op_hashes_present<'e, E>(
    executor: E,
    hashes: &[DhtOpHash],
) -> sqlx::Result<Vec<bool>>
where
    E: Executor<'e, Database = Sqlite> + Copy,
{
    let mut out = Vec::with_capacity(hashes.len());
    for h in hashes {
        out.push(op_exists(executor, h).await?);
    }
    Ok(out)
}
