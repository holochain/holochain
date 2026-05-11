//! Free-standing operations against the `ChainLock` table.

use crate::models::dht::ChainLockRow;
use holo_hash::AgentPubKey;
use holochain_timestamp::Timestamp;
use sqlx::{Executor, Sqlite};

/// Try to acquire the chain lock for `author`.
///
/// Succeeds (returns `Ok(true)`) when there is no existing lock, when the
/// existing lock has expired (relative to `now`), or when the existing lock's
/// `subject` matches — the last case lets the current holder extend or
/// re-acquire their own lock.
///
/// Returns `Ok(false)` when a different subject still holds an unexpired lock,
/// leaving the existing row untouched. This prevents silent lock stealing.
pub(crate) async fn acquire_chain_lock<'e, E>(
    executor: E,
    author: &AgentPubKey,
    subject: &[u8],
    expires_at: Timestamp,
    now: Timestamp,
) -> sqlx::Result<bool>
where
    E: Executor<'e, Database = Sqlite>,
{
    let rows_affected = sqlx::query(
        "INSERT INTO ChainLock (author, subject, expires_at_timestamp)
         VALUES (?1, ?2, ?3)
         ON CONFLICT(author) DO UPDATE SET
            subject = excluded.subject,
            expires_at_timestamp = excluded.expires_at_timestamp
         WHERE ChainLock.expires_at_timestamp <= ?4
            OR ChainLock.subject = excluded.subject",
    )
    .bind(author.get_raw_36())
    .bind(subject)
    .bind(expires_at.as_micros())
    .bind(now.as_micros())
    .execute(executor)
    .await?
    .rows_affected();
    Ok(rows_affected > 0)
}

pub(crate) async fn get_chain_lock<'e, E>(
    executor: E,
    author: AgentPubKey,
    now: Timestamp,
) -> sqlx::Result<Option<ChainLockRow>>
where
    E: Executor<'e, Database = Sqlite>,
{
    sqlx::query_as(
        "SELECT author, subject, expires_at_timestamp FROM ChainLock
         WHERE author = ? AND expires_at_timestamp > ?",
    )
    .bind(author.get_raw_36())
    .bind(now.as_micros())
    .fetch_optional(executor)
    .await
}

pub(crate) async fn release_chain_lock<'e, E>(executor: E, author: &AgentPubKey) -> sqlx::Result<()>
where
    E: Executor<'e, Database = Sqlite>,
{
    sqlx::query("DELETE FROM ChainLock WHERE author = ?")
        .bind(author.get_raw_36())
        .execute(executor)
        .await?;
    Ok(())
}

pub(crate) async fn prune_expired_chain_locks<'e, E>(
    executor: E,
    now: Timestamp,
) -> sqlx::Result<()>
where
    E: Executor<'e, Database = Sqlite>,
{
    sqlx::query("DELETE FROM ChainLock WHERE expires_at_timestamp <= ?")
        .bind(now.as_micros())
        .execute(executor)
        .await?;
    Ok(())
}
