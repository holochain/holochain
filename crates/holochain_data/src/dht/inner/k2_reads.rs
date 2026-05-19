//! K2 op-store reads that span the `ChainOp`/`Warrant`/`Action`/`Entry` tables.
//!
//! These are the cross-table queries Kitsune2's `OpStore` trait expects.
//! Each `async fn` is generic over `sqlx::Executor` so it can run against a
//! pool (`DbRead`/`DbWrite`) or a transaction (`TxRead`/`TxWrite`).
//!
//! All "gossip-shaped" reads (time-slice, ids-since, ops-by-id, presence,
//! earliest-timestamp) filter `ChainOp.locally_validated = 1` so network-
//! cached ops are never gossiped or served to peers. Warrants are not
//! filtered: they always route through `LimboWarrant` validation before
//! being promoted to `Warrant`, so every row in `Warrant` is locally
//! validated by construction.
//!
//! `count_integrated_ops` is the exception: it counts every row in both
//! tables (cache included) to preserve the K2 `query_total_op_count`
//! semantics of "everything we hold locally".

use crate::models::dht::{
    K2ChainOpForWireRow, K2OpHashRow, K2OpIdSinceRow, K2OpPresentRow, K2WarrantForWireRow,
};
use sqlx::{Executor, QueryBuilder, Sqlite};

/// Inclusive `[storage_start_loc, storage_end_loc]` arc bounds.
#[derive(Debug, Clone, Copy)]
pub struct ArcBounds {
    /// Inclusive lower bound on `storage_center_loc`.
    pub start: u32,
    /// Inclusive upper bound on `storage_center_loc`.
    pub end: u32,
}

impl ArcBounds {
    fn start_i64(self) -> i64 {
        self.start as i64
    }
    fn end_i64(self) -> i64 {
        self.end as i64
    }
}

/// Return `(hash, basis, size)` for every integrated, locally-validated op
/// whose authored timestamp falls in `[t_start_micros, t_end_micros)`.
///
/// "Authored timestamp" comes from `Action.timestamp` for chain ops and
/// `Warrant.timestamp` for warrants. Results are ordered by authored
/// timestamp ascending.
pub(crate) async fn op_hashes_in_time_slice<'e, E>(
    executor: E,
    arc: ArcBounds,
    t_start_micros: i64,
    t_end_micros: i64,
) -> sqlx::Result<Vec<K2OpHashRow>>
where
    E: Executor<'e, Database = Sqlite>,
{
    // Two arc-filter shapes: non-wrapping (start <= end) keeps everything
    // inside the range; wrapping (start > end) keeps everything outside.
    // We bind the same arc values twice (once per UNION branch).
    let sql = "
        SELECT op_hash AS hash, basis_hash, serialized_size, sort_ts FROM (
            SELECT
                ChainOp.hash AS op_hash,
                ChainOp.basis_hash AS basis_hash,
                ChainOp.serialized_size AS serialized_size,
                Action.timestamp AS sort_ts
            FROM ChainOp
            JOIN Action ON ChainOp.action_hash = Action.hash
            WHERE
                (
                    (? <= ? AND ChainOp.storage_center_loc >= ?
                            AND ChainOp.storage_center_loc <= ?)
                    OR
                    (? >  ? AND (ChainOp.storage_center_loc <= ?
                              OR ChainOp.storage_center_loc >= ?))
                )
                AND Action.timestamp >= ?
                AND Action.timestamp <  ?
                AND ChainOp.locally_validated = 1
            UNION ALL
            SELECT
                Warrant.hash AS op_hash,
                Warrant.warrantee AS basis_hash,
                Warrant.serialized_size AS serialized_size,
                Warrant.timestamp AS sort_ts
            FROM Warrant
            WHERE
                (
                    (? <= ? AND Warrant.storage_center_loc >= ?
                            AND Warrant.storage_center_loc <= ?)
                    OR
                    (? >  ? AND (Warrant.storage_center_loc <= ?
                              OR Warrant.storage_center_loc >= ?))
                )
                AND Warrant.timestamp >= ?
                AND Warrant.timestamp <  ?
        )
        ORDER BY sort_ts ASC
    ";

    let s = arc.start_i64();
    let e = arc.end_i64();
    sqlx::query_as::<_, K2OpHashRow>(sql)
        // chain-op branch
        .bind(s)
        .bind(e)
        .bind(s)
        .bind(e)
        .bind(s)
        .bind(e)
        .bind(e)
        .bind(s)
        .bind(t_start_micros)
        .bind(t_end_micros)
        // warrant branch
        .bind(s)
        .bind(e)
        .bind(s)
        .bind(e)
        .bind(s)
        .bind(e)
        .bind(e)
        .bind(s)
        .bind(t_start_micros)
        .bind(t_end_micros)
        .fetch_all(executor)
        .await
}

/// Return up to `limit` ops with `when_integrated >= t_min_micros` in
/// integration-time order. Used by the K2 gossip "since" cursor.
pub(crate) async fn op_ids_since_time_batch<'e, E>(
    executor: E,
    arc: ArcBounds,
    t_min_micros: i64,
    limit: u32,
) -> sqlx::Result<Vec<K2OpIdSinceRow>>
where
    E: Executor<'e, Database = Sqlite>,
{
    let sql = "
        SELECT op_hash AS hash, basis_hash, when_integrated, serialized_size FROM (
            SELECT
                ChainOp.hash AS op_hash,
                ChainOp.basis_hash AS basis_hash,
                ChainOp.when_integrated AS when_integrated,
                ChainOp.serialized_size AS serialized_size
            FROM ChainOp
            WHERE
                (
                    (? <= ? AND ChainOp.storage_center_loc >= ?
                            AND ChainOp.storage_center_loc <= ?)
                    OR
                    (? >  ? AND (ChainOp.storage_center_loc <= ?
                              OR ChainOp.storage_center_loc >= ?))
                )
                AND ChainOp.when_integrated >= ?
                AND ChainOp.locally_validated = 1
            UNION ALL
            SELECT
                Warrant.hash AS op_hash,
                Warrant.warrantee AS basis_hash,
                Warrant.when_integrated AS when_integrated,
                Warrant.serialized_size AS serialized_size
            FROM Warrant
            WHERE
                (
                    (? <= ? AND Warrant.storage_center_loc >= ?
                            AND Warrant.storage_center_loc <= ?)
                    OR
                    (? >  ? AND (Warrant.storage_center_loc <= ?
                              OR Warrant.storage_center_loc >= ?))
                )
                AND Warrant.when_integrated >= ?
        )
        ORDER BY when_integrated ASC
        LIMIT ?
    ";

    let s = arc.start_i64();
    let e = arc.end_i64();
    sqlx::query_as::<_, K2OpIdSinceRow>(sql)
        // chain-op branch
        .bind(s)
        .bind(e)
        .bind(s)
        .bind(e)
        .bind(s)
        .bind(e)
        .bind(e)
        .bind(s)
        .bind(t_min_micros)
        // warrant branch
        .bind(s)
        .bind(e)
        .bind(s)
        .bind(e)
        .bind(s)
        .bind(e)
        .bind(e)
        .bind(s)
        .bind(t_min_micros)
        .bind(limit as i64)
        .fetch_all(executor)
        .await
}

/// Return the subset of `hashes` that exist in `ChainOp` (with
/// `locally_validated = 1`) or in `Warrant`, with their basis hashes.
pub(crate) async fn check_op_hashes_present<'e, E>(
    executor: E,
    hashes: &[Vec<u8>],
) -> sqlx::Result<Vec<K2OpPresentRow>>
where
    E: Executor<'e, Database = Sqlite>,
{
    if hashes.is_empty() {
        return Ok(Vec::new());
    }

    // sqlx doesn't expand `IN (...)` for blob slices directly. Build a
    // UNION ALL across ChainOp (filtered to `locally_validated = 1`) and
    // Warrant in a single query, parameterising each hash list.
    let mut qb: QueryBuilder<Sqlite> = QueryBuilder::new(
        "SELECT hash, basis_hash FROM ChainOp
         WHERE locally_validated = 1 AND hash IN (",
    );
    {
        let mut sep = qb.separated(", ");
        for h in hashes {
            sep.push_bind(h);
        }
    }
    qb.push(") UNION ALL SELECT hash, warrantee AS basis_hash FROM Warrant WHERE hash IN (");
    {
        let mut sep = qb.separated(", ");
        for h in hashes {
            sep.push_bind(h);
        }
    }
    qb.push(")");

    qb.build_query_as::<K2OpPresentRow>()
        .fetch_all(executor)
        .await
}

/// Fetch full chain-op rows (joined with `Action` and optional `Entry`) for
/// the given op hashes, filtered to `locally_validated = 1`.
pub(crate) async fn get_chain_ops_for_wire<'e, E>(
    executor: E,
    op_hashes: &[Vec<u8>],
) -> sqlx::Result<Vec<K2ChainOpForWireRow>>
where
    E: Executor<'e, Database = Sqlite>,
{
    if op_hashes.is_empty() {
        return Ok(Vec::new());
    }

    let mut qb: QueryBuilder<Sqlite> = QueryBuilder::new(
        "SELECT
            ChainOp.hash AS op_hash,
            ChainOp.basis_hash AS basis_hash,
            ChainOp.op_type AS op_type,
            Action.hash AS action_hash,
            Action.author AS author,
            Action.timestamp AS timestamp,
            Action.seq AS seq,
            Action.prev_hash AS prev_hash,
            Action.action_data AS action_data,
            Action.signature AS signature,
            Entry.blob AS entry_blob
         FROM ChainOp
         JOIN Action ON ChainOp.action_hash = Action.hash
         LEFT JOIN Entry ON Action.entry_hash = Entry.hash
         WHERE ChainOp.locally_validated = 1
           AND ChainOp.hash IN (",
    );
    {
        let mut sep = qb.separated(", ");
        for h in op_hashes {
            sep.push_bind(h);
        }
    }
    qb.push(")");

    qb.build_query_as::<K2ChainOpForWireRow>()
        .fetch_all(executor)
        .await
}

/// Fetch full warrant rows for the given op hashes.
pub(crate) async fn get_warrants_for_wire<'e, E>(
    executor: E,
    op_hashes: &[Vec<u8>],
) -> sqlx::Result<Vec<K2WarrantForWireRow>>
where
    E: Executor<'e, Database = Sqlite>,
{
    if op_hashes.is_empty() {
        return Ok(Vec::new());
    }

    let mut qb: QueryBuilder<Sqlite> = QueryBuilder::new(
        "SELECT hash, author, timestamp, warrantee, proof, signature
         FROM Warrant WHERE hash IN (",
    );
    {
        let mut sep = qb.separated(", ");
        for h in op_hashes {
            sep.push_bind(h);
        }
    }
    qb.push(")");

    qb.build_query_as::<K2WarrantForWireRow>()
        .fetch_all(executor)
        .await
}

/// Minimum authored timestamp across both `ChainOp` (joined with `Action`)
/// and `Warrant`, filtered to `arc` and (for chain ops)
/// `locally_validated = 1`. `None` when no rows match.
pub(crate) async fn earliest_authored_timestamp_in_arc<'e, E>(
    executor: E,
    arc: ArcBounds,
) -> sqlx::Result<Option<i64>>
where
    E: Executor<'e, Database = Sqlite>,
{
    let sql = "
        SELECT MIN(ts) FROM (
            SELECT Action.timestamp AS ts
            FROM ChainOp
            JOIN Action ON ChainOp.action_hash = Action.hash
            WHERE
                (
                    (? <= ? AND ChainOp.storage_center_loc >= ?
                            AND ChainOp.storage_center_loc <= ?)
                    OR
                    (? >  ? AND (ChainOp.storage_center_loc <= ?
                              OR ChainOp.storage_center_loc >= ?))
                )
                AND ChainOp.when_integrated IS NOT NULL
                AND ChainOp.locally_validated = 1
            UNION ALL
            SELECT Warrant.timestamp AS ts
            FROM Warrant
            WHERE
                (
                    (? <= ? AND Warrant.storage_center_loc >= ?
                            AND Warrant.storage_center_loc <= ?)
                    OR
                    (? >  ? AND (Warrant.storage_center_loc <= ?
                              OR Warrant.storage_center_loc >= ?))
                )
        )
    ";
    let s = arc.start_i64();
    let e = arc.end_i64();
    let row: Option<(Option<i64>,)> = sqlx::query_as(sql)
        // chain-op branch
        .bind(s)
        .bind(e)
        .bind(s)
        .bind(e)
        .bind(s)
        .bind(e)
        .bind(e)
        .bind(s)
        // warrant branch
        .bind(s)
        .bind(e)
        .bind(s)
        .bind(e)
        .bind(s)
        .bind(e)
        .bind(e)
        .bind(s)
        .fetch_optional(executor)
        .await?;
    Ok(row.and_then(|(v,)| v))
}

/// Total count of every integrated op + warrant in this DHT store, with no
/// `locally_validated` filter. Preserves the K2 `query_total_op_count`
/// semantics of "everything we hold locally" (cache included, since the
/// cache mirror writes into `ChainOp`).
pub(crate) async fn count_integrated_ops<'e, E>(executor: E) -> sqlx::Result<i64>
where
    E: Executor<'e, Database = Sqlite>,
{
    let (n,): (i64,) = sqlx::query_as(
        "SELECT
            (SELECT COUNT(*) FROM ChainOp WHERE when_integrated IS NOT NULL)
            +
            (SELECT COUNT(*) FROM Warrant)",
    )
    .fetch_one(executor)
    .await?;
    Ok(n)
}
