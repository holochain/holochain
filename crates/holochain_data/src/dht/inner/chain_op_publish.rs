//! Free-standing operations against the `ChainOpPublish` table.

use crate::models::dht::{ChainOpPublishRow, OpToPublishRow};
use holo_hash::{AgentPubKey, DhtOpHash};
use holochain_timestamp::Timestamp;
use sqlx::{Executor, Sqlite};

pub(crate) async fn insert_chain_op_publish<'e, E>(
    executor: E,
    op_hash: &DhtOpHash,
    last_publish_time: Option<Timestamp>,
    receipts_complete: Option<bool>,
    withhold_publish: Option<bool>,
) -> sqlx::Result<()>
where
    E: Executor<'e, Database = Sqlite>,
{
    sqlx::query(
        "INSERT INTO ChainOpPublish (op_hash, last_publish_time, receipts_complete, withhold_publish)
         VALUES (?, ?, ?, ?)",
    )
    .bind(op_hash.get_raw_36())
    .bind(last_publish_time.map(|t| t.as_micros()))
    .bind(receipts_complete.map(|b| b as i64))
    .bind(withhold_publish.map(|b| b as i64))
    .execute(executor)
    .await?;
    Ok(())
}

pub(crate) async fn get_chain_op_publish<'e, E>(
    executor: E,
    op_hash: DhtOpHash,
) -> sqlx::Result<Option<ChainOpPublishRow>>
where
    E: Executor<'e, Database = Sqlite>,
{
    sqlx::query_as(
        "SELECT op_hash, last_publish_time, receipts_complete, withhold_publish
         FROM ChainOpPublish WHERE op_hash = ?",
    )
    .bind(op_hash.get_raw_36())
    .fetch_optional(executor)
    .await
}

/// Mark receipts as complete for the given op. Receipts can never transition
/// back to incomplete, so no flag is needed.
pub(crate) async fn set_receipts_complete<'e, E>(
    executor: E,
    op_hash: &DhtOpHash,
) -> sqlx::Result<u64>
where
    E: Executor<'e, Database = Sqlite>,
{
    let result = sqlx::query("UPDATE ChainOpPublish SET receipts_complete = 1 WHERE op_hash = ?")
        .bind(op_hash.get_raw_36())
        .execute(executor)
        .await?;
    Ok(result.rows_affected())
}

/// Update `last_publish_time` for the given op. Returns the number of rows updated.
pub(crate) async fn set_last_publish_time<'e, E>(
    executor: E,
    op_hash: &DhtOpHash,
    when: Timestamp,
) -> sqlx::Result<u64>
where
    E: Executor<'e, Database = Sqlite>,
{
    let result = sqlx::query("UPDATE ChainOpPublish SET last_publish_time = ? WHERE op_hash = ?")
        .bind(when.as_micros())
        .bind(op_hash.get_raw_36())
        .execute(executor)
        .await?;
    Ok(result.rows_affected())
}

/// Clear `withhold_publish` (set to NULL) for the given op. Returns the number of rows updated.
pub(crate) async fn clear_withhold_publish<'e, E>(
    executor: E,
    op_hash: &DhtOpHash,
) -> sqlx::Result<u64>
where
    E: Executor<'e, Database = Sqlite>,
{
    let result = sqlx::query("UPDATE ChainOpPublish SET withhold_publish = NULL WHERE op_hash = ?")
        .bind(op_hash.get_raw_36())
        .execute(executor)
        .await?;
    Ok(result.rows_affected())
}

/// Ops eligible to be published to the network for `author`.
///
/// Returns integrated, self-authored chain ops together with integrated
/// warrants authored by `author`, ordered for stable publishing. The filters
/// mirror the historical authored-database query:
///
/// - **Private entries never leave the device.** `StoreEntry` ops (`op_type =
///   2`) whose action carries a private entry (`Action.private_entry = 1`) are
///   excluded. All other op types are published even for private entries
///   because they do not contain the entry data.
/// - Ops with `withhold_publish` set (e.g. in-flight countersigning) are
///   skipped.
/// - Ops published within the minimum publish interval are skipped via
///   `recency_threshold_micros` (an op qualifies when it has never been
///   published or was last published at or before the threshold).
/// - Ops whose receipts are already complete are skipped.
///
/// Warrants publish at most once: a warrant qualifies only while it has no
/// `WarrantPublish` row recording a publish time. Warrants sort last (their
/// `sort_seq` is `i64::MAX`) and their basis is the warrantee.
pub(crate) async fn get_ops_to_publish<'e, E>(
    executor: E,
    author: &AgentPubKey,
    recency_threshold_micros: i64,
) -> sqlx::Result<Vec<OpToPublishRow>>
where
    E: Executor<'e, Database = Sqlite>,
{
    sqlx::query_as(
        "SELECT ChainOp.hash AS dht_hash, ChainOp.basis_hash AS basis_hash, Action.seq AS sort_seq
         FROM ChainOp
         JOIN Action ON ChainOp.action_hash = Action.hash
         LEFT JOIN ChainOpPublish ON ChainOpPublish.op_hash = ChainOp.hash
         WHERE Action.author = ?
           AND (ChainOp.op_type != 2 OR Action.private_entry = 0)
           AND ChainOpPublish.withhold_publish IS NULL
           AND (ChainOpPublish.last_publish_time IS NULL
                OR ChainOpPublish.last_publish_time <= ?)
           AND ChainOpPublish.receipts_complete IS NULL

         UNION ALL

         SELECT WarrantOp.hash AS dht_hash, Warrant.warrantee AS basis_hash,
                9223372036854775807 AS sort_seq
         FROM WarrantOp
         JOIN Warrant ON WarrantOp.hash = Warrant.hash
         LEFT JOIN WarrantPublish ON WarrantPublish.warrant_hash = Warrant.hash
         WHERE Warrant.author = ?
           AND WarrantPublish.last_publish_time IS NULL

         ORDER BY sort_seq, dht_hash",
    )
    .bind(author.get_raw_36())
    .bind(recency_threshold_micros)
    .bind(author.get_raw_36())
    .fetch_all(executor)
    .await
}

/// Count integrated ops authored by `author` that have been published at
/// least once (i.e. `ChainOpPublish.last_publish_time IS NOT NULL`).
///
/// Used by [`crate::dht`] to compute the `published_ops_count` field in the
/// source-chain dump.
pub(crate) async fn count_published_ops_for_author<'e, E>(
    executor: E,
    author: &AgentPubKey,
) -> sqlx::Result<i64>
where
    E: Executor<'e, Database = Sqlite>,
{
    sqlx::query_scalar(
        "SELECT COUNT(ChainOp.hash) FROM ChainOp
         JOIN Action ON ChainOp.action_hash = Action.hash
         JOIN ChainOpPublish ON ChainOpPublish.op_hash = ChainOp.hash
         WHERE Action.author = ?
           AND ChainOpPublish.last_publish_time IS NOT NULL",
    )
    .bind(author.get_raw_36())
    .fetch_one(executor)
    .await
}

/// Count ops authored by `author` that may still need to be published.
///
/// Like [`get_ops_to_publish`] but ignores the recency window: an op counts as
/// "still needing publish" while it has not had its receipts completed
/// (chain ops) or has never been published (warrants), regardless of how
/// recently it was last published. The publish workflow uses this to decide
/// whether to keep its loop running.
pub(crate) async fn num_still_needing_publish<'e, E>(
    executor: E,
    author: &AgentPubKey,
) -> sqlx::Result<i64>
where
    E: Executor<'e, Database = Sqlite>,
{
    sqlx::query_scalar(
        "SELECT
           (SELECT COUNT(*)
            FROM ChainOp
            JOIN Action ON ChainOp.action_hash = Action.hash
            LEFT JOIN ChainOpPublish ON ChainOpPublish.op_hash = ChainOp.hash
            WHERE Action.author = ?
              AND ChainOpPublish.withhold_publish IS NULL
              AND (ChainOp.op_type != 2 OR Action.private_entry = 0)
              AND ChainOpPublish.receipts_complete IS NULL)
         + (SELECT COUNT(*)
            FROM WarrantOp
            JOIN Warrant ON WarrantOp.hash = Warrant.hash
            LEFT JOIN WarrantPublish ON WarrantPublish.warrant_hash = Warrant.hash
            WHERE Warrant.author = ?
              AND WarrantPublish.last_publish_time IS NULL)",
    )
    .bind(author.get_raw_36())
    .bind(author.get_raw_36())
    .fetch_one(executor)
    .await
}
