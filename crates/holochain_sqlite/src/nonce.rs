//! nonce sql logic
use crate::prelude::*;
use holo_hash::AgentPubKey;
use rusqlite::*;
use crate::sql::sql_nonce;

pub type IntNonce = i64;

pub enum WitnessNonceResult {
    Fresh,
    Stale(IntNonce),
}

pub async fn witness_nonce(db: &DbWrite<DbKindNonce>, agent: AgentPubKey, nonce: IntNonce) -> DatabaseResult<WitnessNonceResult> {
    db.async_commit(move |txn| {
        let maybe_nonce = previously_witnessed_nonce(txn, &agent)?;

        if let Some(previously_witnessed_nonce) = maybe_nonce {
            if nonce <= previously_witnessed_nonce {
                return Ok(WitnessNonceResult::Stale(previously_witnessed_nonce));
            }
        }
        txn.execute(
            sql_nonce::INSERT,
            named_params! {
                ":agent": agent,
                ":nonce": nonce,
            }
        )?;
        Ok(WitnessNonceResult::Fresh)
    }).await
}

pub fn previously_witnessed_nonce(txn: &Transaction<'_>, agent: &AgentPubKey) -> DatabaseResult<Option<IntNonce>> {
    let mut statement = txn.prepare(sql_nonce::SELECT).map_err(|e| rusqlite::Error::ToSqlConversionFailure(e.into()))?;

    Ok(statement.query_row(named_params! {":agent": agent }, |row| {
        Ok(row.get_ref(0)?.as_i64()?)
    }).optional()?)
}

pub async fn fresh_nonce(db: &DbWrite<DbKindNonce>, agent: &AgentPubKey) -> DatabaseResult<IntNonce> {
    db.async_commit(move |txn| {
        let nonce = previously_witnessed_nonce(txn, agent)?.unwrap_or(0) + 1;
        witnes
    })
}