//! nonce sql logic
use crate::prelude::*;
use crate::sql::sql_conductor;
use holo_hash::AgentPubKey;
use holochain_zome_types::Timestamp;
use rusqlite::*;

pub type IntNonce = i64;

pub fn get_nonce(
    txn: &Transaction<'_>,
    agent: &AgentPubKey,
    nonce: IntNonce,
    now: Timestamp,
) -> DatabaseResult<Option<IntNonce>> {
    let mut statement = txn
        .prepare(sql_conductor::SELECT_NONCE)
        .map_err(|e| rusqlite::Error::ToSqlConversionFailure(e.into()))?;

    Ok(statement
        .query_row(
            named_params! {":agent": agent, ":nonce": nonce, ":now": now },
            |row| Ok(row.get_ref(0)?.as_i64()?),
        )
        .optional()?)
}
