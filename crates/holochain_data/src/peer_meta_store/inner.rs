use super::PeerMetaEntry;
use sqlx::{Executor, Sqlite};

/// Insert or replace an arbitrary key-value-pair into the peer metadata store.
///
/// `expires_at` is seconds since the Unix epoch.
pub(super) async fn put<'e, E>(
    executor: E,
    peer_url: &str,
    meta_key: &str,
    meta_value: &[u8],
    expires_at: Option<i64>,
) -> sqlx::Result<()>
where
    E: Executor<'e, Database = Sqlite>,
{
    sqlx::query(
        "INSERT INTO peer_meta (peer_url, meta_key, meta_value, expires_at) VALUES (?, ?, ?, ?)",
    )
    .bind(peer_url)
    .bind(meta_key)
    .bind(meta_value)
    .bind(expires_at)
    .execute(executor)
    .await?;

    Ok(())
}

/// Get the value for a specific peer URL and key, if it exists and has not expired.
pub(super) async fn get<'e, E>(
    executor: E,
    peer_url: &str,
    meta_key: &str,
) -> sqlx::Result<Option<Vec<u8>>>
where
    E: Executor<'e, Database = Sqlite>,
{
    let value: Option<Vec<u8>> = sqlx::query_scalar(
        "SELECT meta_value FROM peer_meta
         WHERE peer_url = ? AND meta_key = ?
         AND (expires_at IS NULL OR expires_at > unixepoch())",
    )
    .bind(peer_url)
    .bind(meta_key)
    .fetch_optional(executor)
    .await?;

    Ok(value)
}

/// Get all non-expired values for a given key, keyed by peer URL.
pub(super) async fn get_all_by_key<'e, E>(
    executor: E,
    meta_key: &str,
) -> sqlx::Result<Vec<(String, Vec<u8>)>>
where
    E: Executor<'e, Database = Sqlite>,
{
    #[derive(sqlx::FromRow)]
    struct Row {
        peer_url: String,
        meta_value: Vec<u8>,
    }

    let rows: Vec<Row> = sqlx::query_as(
        "SELECT peer_url, meta_value FROM peer_meta
         WHERE meta_key = ?
         AND (expires_at IS NULL OR expires_at > unixepoch())",
    )
    .bind(meta_key)
    .fetch_all(executor)
    .await?;

    Ok(rows
        .into_iter()
        .map(|r| (r.peer_url, r.meta_value))
        .collect())
}

/// Get all non-expired metadata entries for a given peer URL.
pub(super) async fn get_all_by_url<'e, E>(
    executor: E,
    peer_url: &str,
) -> sqlx::Result<Vec<PeerMetaEntry>>
where
    E: Executor<'e, Database = Sqlite>,
{
    let entries: Vec<PeerMetaEntry> = sqlx::query_as(
        "SELECT meta_key, meta_value, expires_at FROM peer_meta
         WHERE peer_url = ?
         AND (expires_at IS NULL OR expires_at > unixepoch())",
    )
    .bind(peer_url)
    .fetch_all(executor)
    .await?;

    Ok(entries)
}

/// Delete a specific peer metadata entry.
pub(super) async fn delete<'e, E>(executor: E, peer_url: &str, meta_key: &str) -> sqlx::Result<()>
where
    E: Executor<'e, Database = Sqlite>,
{
    sqlx::query("DELETE FROM peer_meta WHERE peer_url = ? AND meta_key = ?")
        .bind(peer_url)
        .bind(meta_key)
        .execute(executor)
        .await?;

    Ok(())
}

/// Delete all expired entries. Returns the number of rows removed.
pub(super) async fn prune<'e, E>(executor: E) -> sqlx::Result<u64>
where
    E: Executor<'e, Database = Sqlite>,
{
    let result = sqlx::query("DELETE FROM peer_meta WHERE expires_at <= unixepoch()")
        .execute(executor)
        .await?;

    Ok(result.rows_affected())
}
