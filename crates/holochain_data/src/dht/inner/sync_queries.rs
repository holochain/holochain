//! K2 op-store reads that span the
//! `Action`/`Entry`/`ChainOp`/`Warrant`/`WarrantOp` tables.
//!
//! These are the cross-table queries Kitsune2's `OpStore` trait expects.
//! Each `async fn` is generic over `sqlx::Executor` so it can run against a
//! pool (`DbRead`/`DbWrite`) or a transaction (`TxRead`/`TxWrite`).
//!
//! Reads that *serve* ops to peers (time-slice, ids-since, ops-for-wire,
//! earliest-timestamp) filter `ChainOp.locally_validated = 1` so network-
//! cached ops are never gossiped. Warrants need no such filter: they always
//! route through `LimboWarrantOp` validation before being promoted to
//! `WarrantOp`, so every warrant joined against `WarrantOp` is locally
//! validated by construction.
//!
//! `check_op_hashes_present` is the exception: it answers "do we already
//! hold this op, at any stage?" so the fetch logic never re-requests ops
//! that are sitting in limbo or the cache. It matches the limbo and
//! integrated tables alike, with no `locally_validated` filter.
//!
//! `count_integrated_ops` counts every integrated op (`ChainOp` +
//! `WarrantOp`) to report the total observed DHT size.

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
                WarrantOp.serialized_size AS serialized_size,
                Warrant.timestamp AS sort_ts
            FROM Warrant
            JOIN WarrantOp ON WarrantOp.hash = Warrant.hash
            WHERE
                (
                    (? <= ? AND WarrantOp.storage_center_loc >= ?
                            AND WarrantOp.storage_center_loc <= ?)
                    OR
                    (? >  ? AND (WarrantOp.storage_center_loc <= ?
                              OR WarrantOp.storage_center_loc >= ?))
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
                WarrantOp.when_integrated AS when_integrated,
                WarrantOp.serialized_size AS serialized_size
            FROM Warrant
            JOIN WarrantOp ON WarrantOp.hash = Warrant.hash
            WHERE
                (
                    (? <= ? AND WarrantOp.storage_center_loc >= ?
                            AND WarrantOp.storage_center_loc <= ?)
                    OR
                    (? >  ? AND (WarrantOp.storage_center_loc <= ?
                              OR WarrantOp.storage_center_loc >= ?))
                )
                AND WarrantOp.when_integrated >= ?
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

/// Return the subset of `hashes` we already hold locally, with their basis
/// hashes — used to decide which ops still need fetching from peers.
///
/// "Hold locally" means present at *any* stage: a chain op in `LimboChainOp`
/// (awaiting validation) or `ChainOp` (integrated or cache-mirrored), or a
/// warrant whose content row exists in `Warrant` (which by invariant implies
/// a `LimboWarrantOp` or `WarrantOp` row). There is deliberately no
/// `locally_validated` filter: an op we are still validating is one we
/// should not re-request.
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

    // sqlx doesn't expand `IN (...)` for blob slices directly, so build the
    // UNION across the limbo + integrated chain-op tables and the shared
    // warrant content table, binding each hash list in turn.
    let mut qb: QueryBuilder<Sqlite> =
        QueryBuilder::new("SELECT hash, basis_hash FROM LimboChainOp WHERE hash IN (");
    {
        let mut sep = qb.separated(", ");
        for h in hashes {
            sep.push_bind(h);
        }
    }
    qb.push(
        ") UNION
         SELECT hash, basis_hash FROM ChainOp WHERE hash IN (",
    );
    {
        let mut sep = qb.separated(", ");
        for h in hashes {
            sep.push_bind(h);
        }
    }
    qb.push(
        ") UNION
         SELECT hash, warrantee AS basis_hash FROM Warrant WHERE hash IN (",
    );
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

/// Fetch full warrant rows for the given op hashes (integrated warrants
/// only — joined with `WarrantOp`).
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
        "SELECT Warrant.hash, Warrant.author, Warrant.timestamp,
                Warrant.warrantee, Warrant.proof, Warrant.signature
         FROM Warrant
         JOIN WarrantOp ON WarrantOp.hash = Warrant.hash
         WHERE Warrant.hash IN (",
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
/// and integrated warrants (`Warrant` joined with `WarrantOp`), filtered
/// to `arc` and (for chain ops) `locally_validated = 1`. `None` when no
/// rows match.
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
                AND ChainOp.locally_validated = 1
            UNION ALL
            SELECT Warrant.timestamp AS ts
            FROM Warrant
            JOIN WarrantOp ON WarrantOp.hash = Warrant.hash
            WHERE
                (
                    (? <= ? AND WarrantOp.storage_center_loc >= ?
                            AND WarrantOp.storage_center_loc <= ?)
                    OR
                    (? >  ? AND (WarrantOp.storage_center_loc <= ?
                              OR WarrantOp.storage_center_loc >= ?))
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
/// `locally_validated` filter — the total observed DHT size.
pub(crate) async fn count_integrated_ops<'e, E>(executor: E) -> sqlx::Result<i64>
where
    E: Executor<'e, Database = Sqlite>,
{
    let (n,): (i64,) = sqlx::query_as(
        "SELECT
            (SELECT COUNT(*) FROM ChainOp)
            +
            (SELECT COUNT(*) FROM WarrantOp)",
    )
    .fetch_one(executor)
    .await?;
    Ok(n)
}
