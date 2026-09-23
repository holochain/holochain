//! Free-standing operations against the `CapGrant` table.

use super::entry::decode_entry_blob;
use crate::models::dht::CapGrantRow;
use holo_hash::{ActionHash, AgentPubKey, EntryHash};
use holochain_integrity_types::capability::CapGrant;
use holochain_integrity_types::entry::Entry;
use sqlx::{Executor, Sqlite};

/// Reads a capability grant from the author's `PrivateEntry` rows.
///
/// Capability grants are always private entries, so only `PrivateEntry` is
/// consulted, never the public `Entry` table. The row is selected by
/// `(hash, author)`, matching the table's primary key, so another agent's
/// private copy of an identical grant is never returned. A blob under that
/// key that is not an [`Entry::CapGrant`] is reported as
/// [`sqlx::Error::Decode`].
pub(crate) async fn get_cap_grant_entry<'e, E>(
    executor: E,
    entry_hash: &EntryHash,
    author: &AgentPubKey,
) -> sqlx::Result<Option<CapGrant>>
where
    E: Executor<'e, Database = Sqlite>,
{
    let row: Option<(Vec<u8>,)> =
        sqlx::query_as("SELECT blob FROM PrivateEntry WHERE hash = ? AND author = ?")
            .bind(entry_hash.get_raw_36())
            .bind(author.get_raw_36())
            .fetch_optional(executor)
            .await?;
    let Some((blob,)) = row else {
        return Ok(None);
    };
    match decode_entry_blob(&blob)? {
        Entry::CapGrant(grant) => Ok(Some(grant)),
        other => Err(sqlx::Error::Decode(Box::new(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!(
                "expected a CapGrant private entry for {entry_hash}, found {}",
                entry_variant_name(&other)
            ),
        )))),
    }
}

/// Returns the variant name of an entry for error messages.
fn entry_variant_name(entry: &Entry) -> &'static str {
    match entry {
        Entry::Agent(_) => "Agent",
        Entry::App(_) => "App",
        Entry::CounterSign(_, _) => "CounterSign",
        Entry::CapClaim(_) => "CapClaim",
        Entry::CapGrant(_) => "CapGrant",
    }
}

pub(crate) async fn insert_cap_grant<'e, E>(
    executor: E,
    action_hash: &ActionHash,
    cap_access: i64,
    tag: Option<&str>,
) -> sqlx::Result<()>
where
    E: Executor<'e, Database = Sqlite>,
{
    sqlx::query("INSERT INTO CapGrant (action_hash, cap_access, tag) VALUES (?, ?, ?)")
        .bind(action_hash.get_raw_36())
        .bind(cap_access)
        .bind(tag)
        .execute(executor)
        .await?;
    Ok(())
}

pub(crate) async fn get_cap_grants_by_access<'e, E>(
    executor: E,
    author: AgentPubKey,
    cap_access: i64,
) -> sqlx::Result<Vec<CapGrantRow>>
where
    E: Executor<'e, Database = Sqlite>,
{
    sqlx::query_as(
        "SELECT cg.action_hash, cg.cap_access, cg.tag
         FROM CapGrant cg
         JOIN Action ON cg.action_hash = Action.hash
         WHERE cg.cap_access = ? AND Action.author = ?
         ORDER BY Action.seq",
    )
    .bind(cap_access)
    .bind(author.get_raw_36())
    .fetch_all(executor)
    .await
}

pub(crate) async fn get_cap_grants_by_tag<'e, E>(
    executor: E,
    author: AgentPubKey,
    tag: &str,
) -> sqlx::Result<Vec<CapGrantRow>>
where
    E: Executor<'e, Database = Sqlite>,
{
    sqlx::query_as(
        "SELECT cg.action_hash, cg.cap_access, cg.tag
         FROM CapGrant cg
         JOIN Action ON cg.action_hash = Action.hash
         WHERE cg.tag = ? AND Action.author = ?
         ORDER BY Action.seq",
    )
    .bind(tag)
    .bind(author.get_raw_36())
    .fetch_all(executor)
    .await
}
