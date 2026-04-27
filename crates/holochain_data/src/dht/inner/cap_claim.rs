//! Free-standing operations against the `CapClaim` table.

use crate::models::dht::CapClaimRow;
use holo_hash::AgentPubKey;
use sqlx::{Executor, Sqlite};

pub(crate) async fn insert_cap_claim<'e, E>(
    executor: E,
    author: &AgentPubKey,
    tag: &str,
    grantor: &AgentPubKey,
    secret: &[u8],
) -> sqlx::Result<()>
where
    E: Executor<'e, Database = Sqlite>,
{
    sqlx::query("INSERT INTO CapClaim (author, tag, grantor, secret) VALUES (?, ?, ?, ?)")
        .bind(author.get_raw_36())
        .bind(tag)
        .bind(grantor.get_raw_36())
        .bind(secret)
        .execute(executor)
        .await?;
    Ok(())
}

pub(crate) async fn get_cap_claims_by_grantor<'e, E>(
    executor: E,
    author: AgentPubKey,
    grantor: AgentPubKey,
) -> sqlx::Result<Vec<CapClaimRow>>
where
    E: Executor<'e, Database = Sqlite>,
{
    sqlx::query_as(
        "SELECT id, author, tag, grantor, secret FROM CapClaim
         WHERE author = ? AND grantor = ? ORDER BY id",
    )
    .bind(author.get_raw_36())
    .bind(grantor.get_raw_36())
    .fetch_all(executor)
    .await
}

pub(crate) async fn get_cap_claims_by_tag<'e, E>(
    executor: E,
    author: AgentPubKey,
    tag: &str,
) -> sqlx::Result<Vec<CapClaimRow>>
where
    E: Executor<'e, Database = Sqlite>,
{
    sqlx::query_as(
        "SELECT id, author, tag, grantor, secret FROM CapClaim
         WHERE author = ? AND tag = ? ORDER BY id",
    )
    .bind(author.get_raw_36())
    .bind(tag)
    .fetch_all(executor)
    .await
}
