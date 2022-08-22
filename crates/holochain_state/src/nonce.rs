use crate::mutations;
use holochain_sqlite::nonce::get_nonce;
use holochain_sqlite::nonce::IntNonce;
use holochain_sqlite::prelude::DatabaseResult;
use holochain_sqlite::prelude::DbWrite;
use holochain_sqlite::rusqlite::named_params;
use holochain_sqlite::sql::sql_conductor;
use holochain_types::prelude::AgentPubKey;
use holochain_types::prelude::DbKindConductor;
use holochain_zome_types::Timestamp;
use std::time::Duration;
use rand::Rng;

/// Rather arbitrary but we expire nonces after 5 mins.
pub const FRESH_NONCE_EXPIRES_AFTER: Duration = Duration::from_secs(60 * 5);

#[derive(PartialEq)]
pub enum WitnessNonceResult {
    Fresh,
    Stale,
}

pub async fn witness_nonce(
    db: &DbWrite<DbKindConductor>,
    agent: AgentPubKey,
    nonce: IntNonce,
    now: Timestamp,
    expires: Timestamp,
) -> DatabaseResult<WitnessNonceResult> {
    if expires <= now {
        Ok(WitnessNonceResult::Stale)
    } else {
        db.async_commit(move |txn| {
            txn.execute(
                sql_conductor::DELETE_EXPIRED_NONCE,
                named_params! {":now": now},
            )?;
            if let Some(_) = get_nonce(txn, &agent, nonce, now)? {
                Ok(WitnessNonceResult::Stale)
            } else {
                mutations::insert_nonce(txn, &agent, nonce, expires)?;
                Ok(WitnessNonceResult::Fresh)
            }
        })
        .await
    }
}

pub async fn fresh_nonce(
    now: Timestamp,
) -> DatabaseResult<(IntNonce, Timestamp)> {
    // very unlikely to get a collision, we assume impossible.
    let nonce = rand::thread_rng().gen();
    let expires: Timestamp = (now + FRESH_NONCE_EXPIRES_AFTER)?;
    Ok((nonce, expires))
}
