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
//! hold this op in a form that doesn't need re-delivery?" so the fetch logic
//! never re-requests ops that are already in the validation pipeline. It
//! matches `LimboChainOp` (awaiting validation) and `ChainOp` with
//! `locally_validated = 1` (integrated), but *not* cache-mirrored ops
//! (`locally_validated = 0`), which still rely on gossip to re-deliver them
//! into validation.
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

/// Return the subset of `hashes` we already hold in a way that does not need
/// re-delivery, with their basis hashes — used to decide which ops still
/// need fetching from peers.
///
/// "Hold" means a chain op that is either awaiting validation in
/// `LimboChainOp` or already integrated by us (`ChainOp` with
/// `locally_validated = 1`), or a warrant whose content row exists in
/// `Warrant` (which by invariant implies a `LimboWarrantOp` or `WarrantOp`
/// row, both of which route through validation).
///
/// Cache-mirrored ops (`ChainOp` with `locally_validated = 0`) are
/// deliberately *excluded*: their content is held only to serve reads, they
/// never entered the validation pipeline, and the only way they reach it is
/// to be re-delivered by gossip. Reporting them as present would suppress
/// that re-delivery and they would never integrate.
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
    // UNION across the limbo + locally-validated chain-op tables and the
    // shared warrant content table, binding each hash list in turn.
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
         SELECT hash, basis_hash FROM ChainOp WHERE locally_validated = 1 AND hash IN (",
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

/// Fetch every locally-validated (integrated) chain-op row, joined with
/// `Action` and optional `Entry`, for full wire reconstruction. Same columns
/// as [`get_chain_ops_for_wire`] but with no hash filter. Ordered by
/// `ChainOp.hash` for a stable result (`ChainOp` is `WITHOUT ROWID`, so
/// `rowid` is unavailable).
pub(crate) async fn all_integrated_chain_ops_for_wire<'e, E>(
    executor: E,
) -> sqlx::Result<Vec<K2ChainOpForWireRow>>
where
    E: Executor<'e, Database = Sqlite>,
{
    sqlx::query_as::<_, K2ChainOpForWireRow>(
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
         ORDER BY ChainOp.hash ASC",
    )
    .fetch_all(executor)
    .await
}

/// Fetch limbo chain-op rows for full wire reconstruction, joined with
/// `Action` and optional `Entry`. Same columns as [`get_chain_ops_for_wire`]
/// but sourced from `LimboChainOp`. When `ready` is true only rows matching
/// [`LIMBO_CHAIN_OP_READY_PRED`] are returned (integration-limbo); when false
/// only the complement is returned (validation-limbo). Ordered by
/// `LimboChainOp.hash` for a stable result (`LimboChainOp` is `WITHOUT
/// ROWID`).
pub(crate) async fn limbo_chain_ops_for_wire<'e, E>(
    executor: E,
    ready: bool,
) -> sqlx::Result<Vec<K2ChainOpForWireRow>>
where
    E: Executor<'e, Database = Sqlite>,
{
    let predicate = if ready {
        LIMBO_CHAIN_OP_READY_PRED.to_string()
    } else {
        limbo_chain_op_not_ready_pred()
    };
    let sql = format!(
        "SELECT
            LimboChainOp.hash AS op_hash,
            LimboChainOp.basis_hash AS basis_hash,
            LimboChainOp.op_type AS op_type,
            Action.hash AS action_hash,
            Action.author AS author,
            Action.timestamp AS timestamp,
            Action.seq AS seq,
            Action.prev_hash AS prev_hash,
            Action.action_data AS action_data,
            Action.signature AS signature,
            Entry.blob AS entry_blob
         FROM LimboChainOp
         JOIN Action ON LimboChainOp.action_hash = Action.hash
         LEFT JOIN Entry ON Action.entry_hash = Entry.hash
         WHERE {predicate}
         ORDER BY LimboChainOp.hash ASC"
    );
    // SQL is assembled from a compile-time-constant predicate (no user
    // input), so asserting it is safe.
    sqlx::query_as::<_, K2ChainOpForWireRow>(sqlx::AssertSqlSafe(sql))
        .fetch_all(executor)
        .await
}

/// Fetch every integrated warrant row for full wire reconstruction. Same
/// columns as [`get_warrants_for_wire`] but with no hash filter. Ordered by
/// `WarrantOp.hash` for a stable result (`WarrantOp` is `WITHOUT ROWID`).
pub(crate) async fn all_integrated_warrants_for_wire<'e, E>(
    executor: E,
) -> sqlx::Result<Vec<K2WarrantForWireRow>>
where
    E: Executor<'e, Database = Sqlite>,
{
    sqlx::query_as::<_, K2WarrantForWireRow>(
        "SELECT Warrant.hash, Warrant.author, Warrant.timestamp,
                Warrant.warrantee, Warrant.proof, Warrant.signature
         FROM Warrant
         JOIN WarrantOp ON WarrantOp.hash = Warrant.hash
         ORDER BY WarrantOp.hash ASC",
    )
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

/// Predicate matching `LimboChainOp` rows ready for integration. Kept in
/// sync with `limbo_chain_ops_ready_for_integration` (in
/// `inner/limbo_chain_op.rs`): a row is ready once sys-validation rejected it
/// outright, or sys-validation accepted it and app-validation reached a
/// terminal state.
const LIMBO_CHAIN_OP_READY_PRED: &str =
    "sys_validation_status = 2 OR (sys_validation_status = 1 AND app_validation_status IN (1, 2))";

/// Negation of [`LIMBO_CHAIN_OP_READY_PRED`]. The ready predicate evaluates
/// to `NULL` (not `false`) for rows with `sys_validation_status IS NULL`, so
/// a bare `NOT (...)` would silently drop pending ops under SQL's
/// three-valued logic. Wrapping in `COALESCE(..., 0)` forces those rows to
/// count as not-ready, which is what "still in validation" means.
fn limbo_chain_op_not_ready_pred() -> String {
    format!("NOT COALESCE(({LIMBO_CHAIN_OP_READY_PRED}), 0)")
}

/// `(validation_limbo, integration_limbo, integrated)` counts across the
/// limbo + integrated tables, matching the legacy integration-state report.
///
/// - `integrated` = locally-validated `ChainOp` rows (GET-cached ops with
///   `locally_validated = 0` are excluded) plus all `WarrantOp` rows.
/// - `integration_limbo` = limbo ops ready for integration: `LimboChainOp`
///   rows matching [`LIMBO_CHAIN_OP_READY_PRED`] plus `LimboWarrantOp` rows
///   with a terminal `sys_validation_status` (1 or 2).
/// - `validation_limbo` = limbo ops not yet ready: the complement of the
///   above within each limbo table.
pub(crate) async fn integration_state_counts<'e, E>(executor: E) -> sqlx::Result<(i64, i64, i64)>
where
    E: Executor<'e, Database = Sqlite>,
{
    let sql = format!(
        "SELECT
            (
                (SELECT COUNT(*) FROM LimboChainOp WHERE {not_ready})
                +
                (SELECT COUNT(*) FROM LimboWarrantOp WHERE sys_validation_status IS NULL)
            ) AS validation_limbo,
            (
                (SELECT COUNT(*) FROM LimboChainOp WHERE {ready})
                +
                (SELECT COUNT(*) FROM LimboWarrantOp WHERE sys_validation_status IN (1, 2))
            ) AS integration_limbo,
            (
                (SELECT COUNT(*) FROM ChainOp WHERE locally_validated = 1)
                +
                (SELECT COUNT(*) FROM WarrantOp)
            ) AS integrated",
        ready = LIMBO_CHAIN_OP_READY_PRED,
        not_ready = limbo_chain_op_not_ready_pred(),
    );
    // SQL is assembled from a compile-time-constant predicate (no user
    // input), so asserting it is safe.
    let counts: (i64, i64, i64) = sqlx::query_as(sqlx::AssertSqlSafe(sql))
        .fetch_one(executor)
        .await?;
    Ok(counts)
}
