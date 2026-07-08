//! Free-standing operations against the `Entry` and `PrivateEntry` tables.

use holo_hash::{AgentPubKey, EntryHash};
use holochain_integrity_types::entry::Entry;
use sqlx::{Executor, QueryBuilder, Sqlite, SqliteConnection};
use std::collections::{HashMap, HashSet};

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
    row.map(|(blob,)| decode_entry_blob(&blob)).transpose()
}

/// Batch-reads entries by hash, mirroring [`get_entry`]'s visibility rules but
/// in a single query per chunk instead of one round-trip per hash.
///
/// Each requested hash resolves to its decoded [`Entry`] when present: a public
/// `Entry` is preferred, and — only when `author` is `Some` — that author's
/// `PrivateEntry` is used as a fallback. Hashes with no matching entry are
/// simply absent from the returned map, so the caller can treat a miss as
/// `None`.
///
/// The input is de-duplicated and chunked so arbitrarily long chains stay well
/// within SQLite's bound-parameter limit (each hash is bound up to twice per
/// query — once for the public branch and once for the private branch).
pub(crate) async fn get_entries_by_hashes(
    executor: &mut SqliteConnection,
    hashes: &[EntryHash],
    author: Option<&AgentPubKey>,
) -> sqlx::Result<HashMap<EntryHash, Entry>> {
    // SQLite caps bound parameters at ~32766; 500 hashes (bound at most twice
    // each, plus the author) stays comfortably under that on every supported
    // build.
    const CHUNK_SIZE: usize = 500;

    // De-duplicate so a hash repeated across actions is fetched only once.
    let mut seen: HashSet<&EntryHash> = HashSet::with_capacity(hashes.len());
    let unique: Vec<&EntryHash> = hashes.iter().filter(|h| seen.insert(h)).collect();

    let mut out: HashMap<EntryHash, Entry> = HashMap::with_capacity(unique.len());
    for chunk in unique.chunks(CHUNK_SIZE) {
        // Public entries first, then — only when an author is supplied — that
        // author's private entries. The ordering reproduces `get_entry`'s
        // `UNION ALL ... LIMIT 1` precedence, where a public `Entry` outranks a
        // same-hash `PrivateEntry`.
        let mut qb: QueryBuilder<Sqlite> =
            QueryBuilder::new("SELECT hash, blob FROM Entry WHERE hash IN (");
        {
            let mut sep = qb.separated(", ");
            for h in chunk {
                sep.push_bind(h.get_raw_36().to_vec());
            }
        }
        qb.push(")");
        if let Some(author) = author {
            qb.push(" UNION ALL SELECT hash, blob FROM PrivateEntry WHERE author = ");
            qb.push_bind(author.get_raw_36().to_vec());
            qb.push(" AND hash IN (");
            {
                let mut sep = qb.separated(", ");
                for h in chunk {
                    sep.push_bind(h.get_raw_36().to_vec());
                }
            }
            qb.push(")");
        }

        let rows: Vec<(Vec<u8>, Vec<u8>)> = qb.build_query_as().fetch_all(&mut *executor).await?;

        for (hash_bytes, blob) in rows {
            let hash = EntryHash::from_raw_36(hash_bytes);
            // Public rows precede private rows in the `UNION ALL`, so a hash
            // already present came from the public `Entry` table and wins —
            // matching `get_entry`'s `LIMIT 1`. Skip before decoding so a
            // shadowed private blob is never deserialized.
            if out.contains_key(&hash) {
                continue;
            }
            out.insert(hash, decode_entry_blob(&blob)?);
        }
    }
    Ok(out)
}

/// Decode an `Entry` blob, mapping a deserialization failure to the same
/// [`sqlx::Error::Decode`] shape that [`get_entry`] uses.
fn decode_entry_blob(blob: &[u8]) -> sqlx::Result<Entry> {
    holochain_serialized_bytes::decode::<_, Entry>(blob).map_err(|e| {
        sqlx::Error::Decode(Box::new(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("decode Entry: {e}"),
        )))
    })
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
