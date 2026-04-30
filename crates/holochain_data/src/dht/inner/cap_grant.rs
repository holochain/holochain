//! Free-standing operations against the `CapGrant` table.

use crate::models::dht::CapGrantRow;
use holo_hash::{ActionHash, AgentPubKey};
use sqlx::{Executor, Sqlite};

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
