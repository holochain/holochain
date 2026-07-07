//! Force-remove a self-authored countersigning session.

use holo_hash::{ActionHash, EntryHash};
use sqlx::SqliteConnection;

/// Outcome of `remove_countersigning_session`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RemoveCountersigningSessionOutcome {
    /// The session's rows were deleted.
    Removed,
    /// Refused: at least one of the session action's ops is already published
    /// (it has a `ChainOpPublish` row with `withhold_publish IS NULL`).
    AlreadyPublished,
}

/// Force-remove a self-authored countersigning session identified by its
/// action hash and entry hash.
///
/// 1. **Published guard.** A self-authored op is "published" when it has a
///    `ChainOpPublish` row whose `withhold_publish` flag has been cleared
///    (`NULL`). If any of the action's ops is published the function
///    makes no changes and returns
///    [`RemoveCountersigningSessionOutcome::AlreadyPublished`]: once a session's
///    ops have been shared with the network it is unacceptable to remove them.
///    Ops with no `ChainOpPublish` row (network-received, not self-authored) are
///    not counted as published by us and never arise for the abandon path.
/// 2. **Delete.** Otherwise it deletes, in foreign-key-safe order, the action's
///    `ChainOpPublish` rows, its `ChainOp` rows, the `Action` row, and the entry
///    from both `Entry` and `PrivateEntry` (a countersign entry may be private).
///    Once the guard has passed, every self-authored op for this action is
///    withheld, so deleting all of the action's `ChainOp` rows removes exactly
///    the session and lets the `Action` row's foreign key be satisfied.
///
/// **The caller must wrap this in a transaction** so the guard and the deletes
/// are applied atomically.
pub(crate) async fn remove_countersigning_session(
    conn: &mut SqliteConnection,
    action_hash: &ActionHash,
    entry_hash: &EntryHash,
) -> sqlx::Result<RemoveCountersigningSessionOutcome> {
    // Published guard: refuse if any self-authored op for this action has had
    // its withhold flag cleared (i.e. it is publishable / published).
    let published: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM ChainOp
         JOIN ChainOpPublish ON ChainOpPublish.op_hash = ChainOp.hash
         WHERE ChainOp.action_hash = ?1 AND ChainOpPublish.withhold_publish IS NULL",
    )
    .bind(action_hash.get_raw_36())
    .fetch_one(&mut *conn)
    .await?;
    if published != 0 {
        return Ok(RemoveCountersigningSessionOutcome::AlreadyPublished);
    }

    // Capture the session action's author before the `Action` row is deleted:
    // countersign entries are shared across counterparties, so the
    // `PrivateEntry` row must be removed for this author only.
    let author: Option<Vec<u8>> = sqlx::query_scalar("SELECT author FROM Action WHERE hash = ?1")
        .bind(action_hash.get_raw_36())
        .fetch_optional(&mut *conn)
        .await?;

    // Delete the publish rows first (FK: ChainOpPublish -> ChainOp), then the
    // ops (FK: ChainOp -> Action), then the action, then the entry from both
    // the public and private tables. Foreign keys do not cascade for these
    // tables, so the order matters.
    sqlx::query(
        "DELETE FROM ChainOpPublish
         WHERE op_hash IN (SELECT hash FROM ChainOp WHERE action_hash = ?1)",
    )
    .bind(action_hash.get_raw_36())
    .execute(&mut *conn)
    .await?;

    sqlx::query("DELETE FROM ChainOp WHERE action_hash = ?1")
        .bind(action_hash.get_raw_36())
        .execute(&mut *conn)
        .await?;

    sqlx::query("DELETE FROM Action WHERE hash = ?1")
        .bind(action_hash.get_raw_36())
        .execute(&mut *conn)
        .await?;

    sqlx::query("DELETE FROM Entry WHERE hash = ?1")
        .bind(entry_hash.get_raw_36())
        .execute(&mut *conn)
        .await?;

    sqlx::query("DELETE FROM PrivateEntry WHERE hash = ?1 AND author = ?2")
        .bind(entry_hash.get_raw_36())
        .bind(author)
        .execute(&mut *conn)
        .await?;

    Ok(RemoveCountersigningSessionOutcome::Removed)
}
