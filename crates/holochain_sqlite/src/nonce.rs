//! nonce sql logic
use crate::prelude::*;
use crate::sql::sql_conductor;
use holo_hash::AgentPubKey;
use rusqlite::*;

pub type IntNonce = i64;

pub enum WitnessNonceResult {
    Fresh,
    Stale(IntNonce),
}

pub async fn witness_nonce(
    db: &DbWrite<DbKindConductor>,
    agent: AgentPubKey,
    nonce: IntNonce,
) -> DatabaseResult<WitnessNonceResult> {
    db.async_commit(move |txn| {
        if let Some(previously_witnessed_nonce) = get_nonce(txn, &agent)? {
            if nonce <= previously_witnessed_nonce {
                return Ok(WitnessNonceResult::Stale(previously_witnessed_nonce));
            }
        }
        set_nonce(txn, &agent, nonce)?;
        Ok(WitnessNonceResult::Fresh)
    })
    .await
}

pub fn get_nonce(txn: &Transaction<'_>, agent: &AgentPubKey) -> DatabaseResult<Option<IntNonce>> {
    let mut statement = txn
        .prepare(sql_conductor::SELECT_NONCE)
        .map_err(|e| rusqlite::Error::ToSqlConversionFailure(e.into()))?;

    Ok(statement
        .query_row(named_params! {":agent": agent }, |row| {
            Ok(row.get_ref(0)?.as_i64()?)
        })
        .optional()?)
}

pub fn set_nonce(
    txn: &Transaction<'_>,
    agent: &AgentPubKey,
    nonce: IntNonce,
) -> DatabaseResult<()> {
    txn.execute(
        sql_conductor::INSERT_NONCE,
        named_params! {
            ":agent": agent,
            ":nonce": nonce,
        },
    )?;
    Ok(())
}

pub async fn fresh_nonce(
    db: &DbWrite<DbKindConductor>,
    agent: AgentPubKey,
) -> DatabaseResult<IntNonce> {
    db.async_commit(move |txn| {
        let nonce = get_nonce(txn, &agent)?.unwrap_or(0) + 1;
        set_nonce(txn, &agent, nonce)?;
        Ok(nonce)
    })
    .await
}
