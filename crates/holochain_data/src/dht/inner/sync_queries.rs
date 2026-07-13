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
//! The op-discovery reads (`op_hashes_in_time_slice`, `op_ids_since_time_batch`),
//! the by-hash content read (`get_chain_ops_for_wire`), and the earliest-data
//! boundary (`earliest_authored_timestamp_in_arc`) additionally exclude
//! `StoreEntry` ops (`op_type = 2`) whose action carries a private entry
//! (`Action.private_entry = 1`), matching [`super::chain_op_publish::get_ops_to_publish`].
//! A private `StoreEntry` op is produced and stored locally so its author can
//! validate their own entry, but it must never be advertised or served to
//! peers: `check_entry_visibility` in `holochain`'s sys-validation rejects any
//! `StoreEntry` op whose action declares a private entry as
//! `PrivateEntryLeaked`, so a peer that received one anyway could never
//! converge on it — leaving it permanently unreconciled between the author's
//! slice hash and every peer's.
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
    DumpChainOpRow, K2ChainOpForWireRow, K2OpHashRow, K2OpIdSinceRow, K2OpPresentRow,
    K2WarrantForWireRow,
};
use holo_hash::AgentPubKey;
#[cfg(any(test, feature = "inspection"))]
use holo_hash::{AnyLinkableHash, DhtOpHash};
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
                AND (ChainOp.op_type != 2 OR Action.private_entry = 0)
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
            JOIN Action ON ChainOp.action_hash = Action.hash
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
                AND (ChainOp.op_type != 2 OR Action.private_entry = 0)
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
/// the given op hashes, filtered to `locally_validated = 1`. A `StoreEntry`
/// op carrying a private entry is excluded even if directly requested by
/// hash — see the module-level doc for why it must never be served.
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
           AND (ChainOp.op_type != 2 OR Action.private_entry = 0)
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

/// Integrated chain-op rows for the integration dump, joined with `Action` and
/// optional `Entry` for full wire reconstruction. Ordered by
/// `(when_integrated, hash)` and, when `after` is given, starting strictly after
/// that cursor — so repeated dumps page forward through newly integrated ops.
/// Passing `after = None` returns every integrated op (the dump's first page,
/// and how the consistency harness reads the full set). Reconstruct each row
/// with `build_chain_dht_op_v2`.
pub(crate) async fn integrated_chain_ops_for_dump<'e, E>(
    executor: E,
    after: Option<(i64, &[u8])>,
) -> sqlx::Result<Vec<DumpChainOpRow>>
where
    E: Executor<'e, Database = Sqlite>,
{
    let mut qb: QueryBuilder<Sqlite> = QueryBuilder::new(
        "SELECT
            ChainOp.hash AS op_hash,
            ChainOp.basis_hash AS basis_hash,
            ChainOp.op_type AS op_type,
            ChainOp.when_integrated AS when_integrated,
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
         WHERE ChainOp.locally_validated = 1",
    );
    if let Some((when_integrated, hash)) = after {
        qb.push(" AND (ChainOp.when_integrated > ");
        qb.push_bind(when_integrated);
        qb.push(" OR (ChainOp.when_integrated = ");
        qb.push_bind(when_integrated);
        qb.push(" AND ChainOp.hash > ");
        qb.push_bind(hash);
        qb.push("))");
    }
    qb.push(" ORDER BY ChainOp.when_integrated ASC, ChainOp.hash ASC");
    qb.build_query_as::<DumpChainOpRow>()
        .fetch_all(executor)
        .await
}

/// Fetch the chain-op rows that `author` has authored and shares with the
/// DHT, joined for full wire reconstruction. Same columns as
/// [`all_integrated_chain_ops_for_wire`] but scoped to a single author and
/// with the private-entry filter applied: `StoreEntry` ops (`op_type = 2`)
/// carrying a private entry are excluded so private entries never leak into
/// the published set. Ordered by `ChainOp.hash` for a stable result.
pub(crate) async fn ops_to_publish_for_wire<'e, E>(
    executor: E,
    author: &AgentPubKey,
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
           AND Action.author = ?
           AND (ChainOp.op_type != 2 OR Action.private_entry = 0)
         ORDER BY ChainOp.hash ASC",
    )
    .bind(author.get_raw_36())
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
pub(crate) async fn limbo_chain_ops_for_dump<'e, E>(
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
pub(crate) async fn integrated_warrants_for_dump<'e, E>(
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
/// to `arc` and (for chain ops) `locally_validated = 1` with private
/// `StoreEntry` ops excluded so a withheld private op never sets the
/// advertised earliest-data boundary. `None` when no rows match.
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
                AND (ChainOp.op_type != 2 OR Action.private_entry = 0)
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
/// limbo + integrated tables.
///
/// - `integrated` = locally-validated `ChainOp` rows (GET-cached ops with
///   `locally_validated = 0` are excluded) plus all `WarrantOp` rows.
/// - `integration_limbo` = limbo ops ready for integration: `LimboChainOp`
///   rows matching [`LIMBO_CHAIN_OP_READY_PRED`] plus `LimboWarrantOp` rows
///   with a terminal `sys_validation_status` (1 or 2).
/// - `validation_limbo` = limbo ops not yet ready: the complement of the
///   above within each limbo table.
pub(crate) async fn limbo_state_counts<'e, E>(executor: E) -> sqlx::Result<(i64, i64, i64)>
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

/// Count of integrated, locally-validated `ChainOp` rows that passed validation
/// (`validation_status = 1`). GET-cached copies (`locally_validated = 0`) and
/// rejected ops are excluded.
#[cfg(any(test, feature = "inspection"))]
pub(crate) async fn count_valid_integrated_ops<'e, E>(executor: E) -> sqlx::Result<i64>
where
    E: Executor<'e, Database = Sqlite>,
{
    let (n,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM ChainOp WHERE locally_validated = 1 AND validation_status = 1",
    )
    .fetch_one(executor)
    .await?;
    Ok(n)
}

/// Count of `LimboChainOp` rows that have passed both sys- and app-validation
/// (`sys_validation_status = 1 AND app_validation_status = 1`) but are not yet
/// integrated.
#[cfg(any(test, feature = "inspection"))]
pub(crate) async fn count_valid_not_integrated_ops<'e, E>(executor: E) -> sqlx::Result<i64>
where
    E: Executor<'e, Database = Sqlite>,
{
    let (n,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM LimboChainOp \
         WHERE sys_validation_status = 1 AND app_validation_status = 1",
    )
    .fetch_one(executor)
    .await?;
    Ok(n)
}

/// Count of not-yet-integrated `LimboChainOp` rows authored by `author`.
#[cfg(any(test, feature = "inspection"))]
pub(crate) async fn count_pending_ops_for_author<'e, E>(
    executor: E,
    author: &AgentPubKey,
) -> sqlx::Result<i64>
where
    E: Executor<'e, Database = Sqlite>,
{
    let (n,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM LimboChainOp \
         JOIN Action ON Action.hash = LimboChainOp.action_hash \
         WHERE Action.author = ?",
    )
    .bind(author.get_raw_36())
    .fetch_one(executor)
    .await?;
    Ok(n)
}

/// Hashes of integrated, locally-validated chain ops that were rejected.
/// GET-cached copies (`locally_validated = 0`) are excluded. Ordered by hash
/// for a stable result.
#[cfg(any(test, feature = "inspection"))]
pub(crate) async fn rejected_integrated_op_hashes<'e, E>(executor: E) -> sqlx::Result<Vec<Vec<u8>>>
where
    E: Executor<'e, Database = Sqlite>,
{
    let rows: Vec<(Vec<u8>,)> = sqlx::query_as(
        "SELECT hash FROM ChainOp \
         WHERE locally_validated = 1 AND validation_status = 2 \
         ORDER BY hash",
    )
    .fetch_all(executor)
    .await?;
    Ok(rows.into_iter().map(|(h,)| h).collect())
}

/// Total count of every op held in this DHT store: integrated `ChainOp` and
/// `WarrantOp` plus their limbo counterparts.
#[cfg(any(test, feature = "inspection"))]
pub(crate) async fn count_all_ops<'e, E>(executor: E) -> sqlx::Result<i64>
where
    E: Executor<'e, Database = Sqlite>,
{
    let (n,): (i64,) = sqlx::query_as(
        "SELECT
            (SELECT COUNT(*) FROM ChainOp)
            + (SELECT COUNT(*) FROM LimboChainOp)
            + (SELECT COUNT(*) FROM WarrantOp)
            + (SELECT COUNT(*) FROM LimboWarrantOp)",
    )
    .fetch_one(executor)
    .await?;
    Ok(n)
}

/// Whether the integrated chain op `op_hash` is flagged as requiring a
/// validation receipt. Returns `false` when the op is not an integrated
/// chain op.
#[cfg(any(test, feature = "inspection"))]
pub(crate) async fn op_requires_receipt<'e, E>(
    executor: E,
    op_hash: &DhtOpHash,
) -> sqlx::Result<bool>
where
    E: Executor<'e, Database = Sqlite>,
{
    let row: Option<(bool,)> = sqlx::query_as("SELECT require_receipt FROM ChainOp WHERE hash = ?")
        .bind(op_hash.get_raw_36())
        .fetch_optional(executor)
        .await?;
    Ok(row.map(|(b,)| b).unwrap_or(false))
}

/// Whether `op_hash` is present in the limbo (not-yet-integrated) chain ops.
#[cfg(any(test, feature = "inspection"))]
pub(crate) async fn limbo_op_exists<'e, E>(executor: E, op_hash: &DhtOpHash) -> sqlx::Result<bool>
where
    E: Executor<'e, Database = Sqlite>,
{
    let (b,): (bool,) = sqlx::query_as("SELECT EXISTS(SELECT 1 FROM LimboChainOp WHERE hash = ?)")
        .bind(op_hash.get_raw_36())
        .fetch_one(executor)
        .await?;
    Ok(b)
}

/// Hashes of limbo chain ops flagged as requiring a validation receipt.
/// Ordered by hash for a stable result.
#[cfg(any(test, feature = "inspection"))]
pub(crate) async fn limbo_op_hashes_requiring_receipt<'e, E>(
    executor: E,
) -> sqlx::Result<Vec<Vec<u8>>>
where
    E: Executor<'e, Database = Sqlite>,
{
    let rows: Vec<(Vec<u8>,)> =
        sqlx::query_as("SELECT hash FROM LimboChainOp WHERE require_receipt = 1 ORDER BY hash")
            .fetch_all(executor)
            .await?;
    Ok(rows.into_iter().map(|(h,)| h).collect())
}

/// Hashes of integrated chain ops with the given DHT `basis`. Ordered by hash
/// for a stable result.
#[cfg(any(test, feature = "inspection"))]
pub(crate) async fn get_ops_at_basis<'e, E>(
    executor: E,
    basis: &AnyLinkableHash,
) -> sqlx::Result<Vec<Vec<u8>>>
where
    E: Executor<'e, Database = Sqlite>,
{
    let rows: Vec<(Vec<u8>,)> = sqlx::query_as(
        "SELECT hash FROM ChainOp WHERE basis_hash = ? AND locally_validated = 1 ORDER BY hash",
    )
    .bind(basis.get_raw_36())
    .fetch_all(executor)
    .await?;
    Ok(rows.into_iter().map(|(h,)| h).collect())
}

/// Count of rows in the public `Entry` table (private entries live in
/// `PrivateEntry` and are not counted here).
#[cfg(any(test, feature = "inspection"))]
pub(crate) async fn count_entries<'e, E>(executor: E) -> sqlx::Result<i64>
where
    E: Executor<'e, Database = Sqlite>,
{
    let (n,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM Entry")
        .fetch_one(executor)
        .await?;
    Ok(n)
}

/// Count of public `Entry` rows whose action carries a private entry. This is
/// always zero: private entries are stored only in `PrivateEntry`, never in the
/// shared `Entry` table.
#[cfg(any(test, feature = "inspection"))]
pub(crate) async fn count_private_entries_in_public_table<'e, E>(executor: E) -> sqlx::Result<i64>
where
    E: Executor<'e, Database = Sqlite>,
{
    let (n,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM Entry \
         JOIN Action ON Action.entry_hash = Entry.hash \
         WHERE Action.private_entry = 1",
    )
    .fetch_one(executor)
    .await?;
    Ok(n)
}

#[cfg(test)]
mod tests {
    use crate::handles::DbWrite;
    use crate::kind::Dht;
    use crate::test_open_db;
    use holo_hash::{AgentPubKey, DnaHash};
    use sqlx::{Pool, Sqlite};
    use std::sync::Arc;

    /// `ChainOpType::StoreEntry` discriminant.
    const STORE_ENTRY: i64 = 2;
    /// `ChainOpType::StoreRecord` discriminant.
    const STORE_RECORD: i64 = 1;
    /// Author shared by every seeded op so the publish read finds them all.
    const AUTHOR: u8 = 7;

    fn dht_id() -> Dht {
        Dht::new(Arc::new(DnaHash::from_raw_36(vec![0u8; 36])))
    }

    /// Insert one integrated, locally-validated chain op with its action, at
    /// storage location 0 so it falls inside a full arc. `op_tag` makes the
    /// op/action/basis hashes unique; `private_entry` is the action's
    /// private-entry flag; `timestamp` is used for both the authored timestamp
    /// and the integration time. Returns the op hash.
    async fn seed_op(
        pool: &Pool<Sqlite>,
        op_tag: u8,
        op_type: i64,
        private_entry: i64,
        timestamp: i64,
    ) -> Vec<u8> {
        let op_hash = vec![op_tag; 36];
        let action_hash = vec![op_tag + 10; 36];
        let basis_hash = vec![op_tag + 20; 36];

        sqlx::query(
            "INSERT INTO Action
                (hash, author, seq, prev_hash, timestamp, action_type,
                 action_data, signature, entry_hash, private_entry, record_validity)
             VALUES (?, ?, 0, NULL, ?, 0, ?, ?, NULL, ?, NULL)",
        )
        .bind(&action_hash)
        .bind(vec![AUTHOR; 36])
        .bind(timestamp)
        .bind(vec![0u8]) // dummy ActionData blob; reads under test never decode it
        .bind(vec![0u8]) // dummy signature blob
        .bind(private_entry)
        .execute(pool)
        .await
        .unwrap();

        sqlx::query(
            "INSERT INTO ChainOp
                (hash, op_type, action_hash, basis_hash, storage_center_loc,
                 validation_status, locally_validated, require_receipt,
                 when_received, when_integrated, serialized_size)
             VALUES (?, ?, ?, ?, 0, 1, 1, 0, ?, ?, 10)",
        )
        .bind(&op_hash)
        .bind(op_type)
        .bind(&action_hash)
        .bind(&basis_hash)
        .bind(timestamp)
        .bind(timestamp)
        .execute(pool)
        .await
        .unwrap();

        op_hash
    }

    /// Seed three integrated ops that exercise the private-entry filter:
    /// a public `StoreEntry` (servable), a private `StoreEntry` (must be
    /// withheld — it is authored *earliest*, at timestamp 1000, to also probe
    /// the earliest-timestamp boundary), and a `StoreRecord` carrying a private
    /// entry (servable, since it withholds the entry body). Returns
    /// `(db, public_store_entry, private_store_entry, private_store_record)`.
    async fn seed_filter_fixture() -> (DbWrite<Dht>, Vec<u8>, Vec<u8>, Vec<u8>) {
        let db = test_open_db(dht_id()).await.unwrap();
        let public = seed_op(db.pool(), 1, STORE_ENTRY, 0, 2000).await;
        let private = seed_op(db.pool(), 2, STORE_ENTRY, 1, 1000).await;
        let record = seed_op(db.pool(), 3, STORE_RECORD, 1, 3000).await;
        (db, public, private, record)
    }

    #[tokio::test]
    async fn time_slice_read_withholds_private_store_entry() {
        let (db, public, private, record) = seed_filter_fixture().await;

        let hashes: Vec<Vec<u8>> = db
            .as_ref()
            .op_hashes_in_time_slice(0, u32::MAX, 0, i64::MAX)
            .await
            .unwrap()
            .into_iter()
            .map(|r| r.hash)
            .collect();

        assert!(hashes.contains(&public), "public StoreEntry must be served");
        assert!(
            hashes.contains(&record),
            "StoreRecord op with a private entry must be served (body withheld)"
        );
        assert!(
            !hashes.contains(&private),
            "private StoreEntry op must never be advertised in a time slice"
        );
    }

    #[tokio::test]
    async fn ids_since_read_withholds_private_store_entry() {
        let (db, public, private, record) = seed_filter_fixture().await;

        let hashes: Vec<Vec<u8>> = db
            .as_ref()
            .op_ids_since_time_batch(0, u32::MAX, 0, 100)
            .await
            .unwrap()
            .into_iter()
            .map(|r| r.hash)
            .collect();

        assert!(hashes.contains(&public), "public StoreEntry must be served");
        assert!(hashes.contains(&record), "StoreRecord op must be served");
        assert!(
            !hashes.contains(&private),
            "private StoreEntry op must never be advertised in the since cursor"
        );
    }

    #[tokio::test]
    async fn for_wire_read_withholds_private_store_entry_even_when_requested() {
        let (db, public, private, record) = seed_filter_fixture().await;

        // Request all three by hash — the private StoreEntry must still be
        // withheld even though it was named explicitly.
        let requested = vec![public.clone(), private.clone(), record.clone()];
        let hashes: Vec<Vec<u8>> = db
            .as_ref()
            .get_chain_ops_for_wire(&requested)
            .await
            .unwrap()
            .into_iter()
            .map(|r| r.op_hash)
            .collect();

        assert!(hashes.contains(&public), "public StoreEntry must be served");
        assert!(hashes.contains(&record), "StoreRecord op must be served");
        assert!(
            !hashes.contains(&private),
            "private StoreEntry op must never be served by hash"
        );
    }

    #[tokio::test]
    async fn earliest_timestamp_ignores_private_store_entry() {
        let (db, _public, _private, _record) = seed_filter_fixture().await;

        let earliest = db
            .as_ref()
            .earliest_authored_timestamp_in_arc(0, u32::MAX)
            .await
            .unwrap();

        // The private StoreEntry is authored at 1000 but is never servable, so
        // it must not set the advertised earliest-data boundary. The earliest
        // servable op is the public StoreEntry at 2000.
        assert_eq!(
            earliest,
            Some(2000),
            "earliest timestamp must reflect the earliest *servable* op, not a withheld private one"
        );
    }

    #[tokio::test]
    async fn ops_to_publish_read_withholds_private_store_entry() {
        let (db, public, private, record) = seed_filter_fixture().await;
        let author = AgentPubKey::from_raw_36(vec![AUTHOR; 36]);

        let hashes: Vec<Vec<u8>> = db
            .as_ref()
            .ops_to_publish_for_wire(&author)
            .await
            .unwrap()
            .into_iter()
            .map(|r| r.op_hash)
            .collect();

        assert!(
            hashes.contains(&public),
            "public StoreEntry must be published"
        );
        assert!(hashes.contains(&record), "StoreRecord op must be published");
        assert!(
            !hashes.contains(&private),
            "private StoreEntry op must never be published"
        );
    }
}
