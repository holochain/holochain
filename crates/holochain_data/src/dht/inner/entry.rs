//! Free-standing operations against the `Entry` and `PrivateEntry` tables.

use holo_hash::{AgentPubKey, EntryHash};
use holochain_integrity_types::entry::Entry;
use sqlx::{Executor, Sqlite};

pub(crate) async fn insert_entry<'e, E>(
    executor: E,
    hash: &EntryHash,
    entry: &Entry,
) -> sqlx::Result<()>
where
    E: Executor<'e, Database = Sqlite>,
{
    let blob = holochain_serialized_bytes::encode(entry)
        .map_err(|e| sqlx::Error::Protocol(format!("encode Entry: {e}")))?;
    sqlx::query("INSERT INTO Entry (hash, blob) VALUES (?, ?)")
        .bind(hash.get_raw_36())
        .bind(blob)
        .execute(executor)
        .await?;
    Ok(())
}

/// Reads an entry. Looks up `Entry` (public) first; if `author` is `Some`,
/// also looks up `PrivateEntry` for that author. Returns the first match.
pub(crate) async fn get_entry<'e, E>(
    executor: E,
    hash: EntryHash,
    author: Option<&AgentPubKey>,
) -> sqlx::Result<Option<Entry>>
where
    E: Executor<'e, Database = Sqlite>,
{
    let author_bytes = author.map(|a| a.get_raw_36().to_vec());
    let row: Option<(Vec<u8>,)> = sqlx::query_as(
        "SELECT blob FROM Entry WHERE hash = ?1
         UNION ALL
         SELECT blob FROM PrivateEntry WHERE hash = ?1 AND ?2 IS NOT NULL AND author = ?2
         LIMIT 1",
    )
    .bind(hash.get_raw_36())
    .bind(author_bytes)
    .fetch_optional(executor)
    .await?;
    row.map(|(blob,)| {
        holochain_serialized_bytes::decode::<_, Entry>(&blob).map_err(|e| {
            sqlx::Error::Decode(Box::new(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("decode Entry: {e}"),
            )))
        })
    })
    .transpose()
}

pub(crate) async fn insert_private_entry<'e, E>(
    executor: E,
    hash: &EntryHash,
    author: &AgentPubKey,
    entry: &Entry,
) -> sqlx::Result<()>
where
    E: Executor<'e, Database = Sqlite>,
{
    let blob = holochain_serialized_bytes::encode(entry)
        .map_err(|e| sqlx::Error::Protocol(format!("encode Entry: {e}")))?;
    sqlx::query("INSERT INTO PrivateEntry (hash, author, blob) VALUES (?, ?, ?)")
        .bind(hash.get_raw_36())
        .bind(author.get_raw_36())
        .bind(blob)
        .execute(executor)
        .await?;
    Ok(())
}
