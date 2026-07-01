//! Existence checks for op hashes across all op-bearing DHT tables.

use holo_hash::DhtOpHash;
use sqlx::{Executor, Sqlite, SqliteConnection};

/// Returns `true` if the given op hash appears in any of the op-bearing
/// tables (`ChainOp`, `LimboChainOp`, `WarrantOp`, `LimboWarrantOp`).
///
/// Used by the incoming-ops workflow to filter ops that have already
/// been recorded locally so we don't re-process duplicates from the
/// network. Cache-only `ChainOp` rows (`locally_validated = 0`) are
/// excluded, so an op mirrored into the cache but not yet validated is
/// still re-delivered into the validation/integration path.
pub(crate) async fn op_exists<'e, E>(executor: E, hash: &DhtOpHash) -> sqlx::Result<bool>
where
    E: Executor<'e, Database = Sqlite>,
{
    let bytes = hash.get_raw_36();
    let row: (i64,) = sqlx::query_as(
        "SELECT EXISTS (
            SELECT 1 FROM ChainOp WHERE locally_validated = 1 AND hash = ?1
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
///
/// Takes a `&mut SqliteConnection` rather than a generic executor because it
/// re-borrows the connection for each per-hash query.
pub(crate) async fn op_hashes_present(
    executor: &mut SqliteConnection,
    hashes: &[DhtOpHash],
) -> sqlx::Result<Vec<bool>> {
    let mut out = Vec::with_capacity(hashes.len());
    for h in hashes {
        out.push(op_exists(&mut *executor, h).await?);
    }
    Ok(out)
}
