//! nonce sql logic
use crate::prelude::*;
use crate::sql::sql_conductor;
use holo_hash::AgentPubKey;
use holochain_zome_types::zome_io::Nonce256Bits;
use holochain_zome_types::Timestamp;
use rusqlite::*;

pub fn nonce_already_seen(
    txn: &Transaction<'_>,
    agent: &AgentPubKey,
    nonce: Nonce256Bits,
    now: Timestamp,
) -> DatabaseResult<bool> {
    let mut statement = txn
        .prepare(sql_conductor::SELECT_NONCE)
        .map_err(|e| rusqlite::Error::ToSqlConversionFailure(e.into()))?;

    Ok(statement
        .query_row(
            named_params! {":agent": agent, ":nonce": nonce.into_inner(), ":now": now },
            |row| Ok(row.get_ref(0)?.as_i64()?),
        )
        .optional()?
        .is_some())
}
