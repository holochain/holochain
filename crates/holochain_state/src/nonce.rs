use crate::mutations;
use holochain_sqlite::nonce::nonce_already_seen;
use holochain_sqlite::prelude::DatabaseResult;
use holochain_sqlite::prelude::DbWrite;
use holochain_sqlite::rusqlite::named_params;
use holochain_sqlite::sql::sql_conductor;
use holochain_types::prelude::AgentPubKey;
use holochain_types::prelude::DbKindConductor;
use holochain_zome_types::zome_io::Nonce256Bits;
use holochain_zome_types::Timestamp;
use std::time::Duration;

/// Rather arbitrary but we expire nonces after 5 mins.
pub const FRESH_NONCE_EXPIRES_AFTER: Duration = Duration::from_secs(60 * 5);
pub const WITNESSABLE_EXPIRY_DURATION: Duration = Duration::from_secs(600 * 5);

#[derive(PartialEq, Debug)]
pub enum WitnessNonceResult {
    Fresh,
    Duplicate,
    Expired,
    Future,
}

pub async fn witness_nonce(
    db: &DbWrite<DbKindConductor>,
    agent: AgentPubKey,
    nonce: Nonce256Bits,
    now: Timestamp,
    expires: Timestamp,
) -> DatabaseResult<WitnessNonceResult> {
    // Treat expired but also very far future expiries as stale as we cannot trust the time in that case.
    if expires <= now {
        Ok(WitnessNonceResult::Expired)
    } else if expires > (now + WITNESSABLE_EXPIRY_DURATION)? {
        Ok(WitnessNonceResult::Future)
    } else {
        db.async_commit(move |txn| {
            txn.execute(
                sql_conductor::DELETE_EXPIRED_NONCE,
                named_params! {":now": now},
            )?;
            if nonce_already_seen(txn, &agent, nonce, now)? {
                Ok(WitnessNonceResult::Duplicate)
            } else {
                mutations::insert_nonce(txn, &agent, nonce, expires)?;
                Ok(WitnessNonceResult::Fresh)
            }
        })
        .await
    }
}

pub fn fresh_nonce(now: Timestamp) -> DatabaseResult<(Nonce256Bits, Timestamp)> {
    let mut bytes = [0; 32];
    getrandom::getrandom(&mut bytes)?;
    let nonce = Nonce256Bits::from(bytes);
    let expires: Timestamp = (now + FRESH_NONCE_EXPIRES_AFTER)?;
    Ok((nonce, expires))
}

#[cfg(test)]
pub mod test {
    use fixt::prelude::*;
    use hdk::prelude::AgentPubKeyFixturator;
    use holochain_zome_types::Timestamp;

    use crate::{
        nonce::{WitnessNonceResult, FRESH_NONCE_EXPIRES_AFTER},
        prelude::test_conductor_db,
    };

    #[test]
    fn test_fresh_nonce() {
        let now = Timestamp::now();
        let (nonce, expires) = super::fresh_nonce(now).unwrap();
        let (nonce_2, expires_2) = super::fresh_nonce(now).unwrap();
        assert!(nonce != nonce_2);
        assert_eq!(expires, expires_2);
        assert_eq!(expires, (now + FRESH_NONCE_EXPIRES_AFTER).unwrap());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_witness_nonce() {
        let db = test_conductor_db();
        let now_0 = Timestamp::now();
        let agent_0 = fixt!(AgentPubKey, Predictable, 0);
        let agent_1 = fixt!(AgentPubKey, Predictable, 1);
        let (nonce_0, expires_0) = super::fresh_nonce(now_0).unwrap();

        // First witnessing should be fresh.
        let witness_0 =
            super::witness_nonce(&db, agent_0.clone(), nonce_0.clone(), now_0, expires_0)
                .await
                .unwrap();

        assert_eq!(witness_0, WitnessNonceResult::Fresh);

        // Second witnessing stale.
        let witness_1 =
            super::witness_nonce(&db, agent_0.clone(), nonce_0.clone(), now_0, expires_0)
                .await
                .unwrap();

        assert_eq!(witness_1, WitnessNonceResult::Duplicate);

        // Different agent is different witnessing even with same params.
        assert_eq!(
            WitnessNonceResult::Fresh,
            super::witness_nonce(&db, agent_1, nonce_0.clone(), now_0, expires_0)
                .await
                .unwrap()
        );

        // New nonce is bad witnessing.
        let now_1 = Timestamp::now();
        let (nonce_1, expires_1) = super::fresh_nonce(now_1).unwrap();

        assert_eq!(
            WitnessNonceResult::Fresh,
            super::witness_nonce(&db, agent_0.clone(), nonce_1.clone(), now_1, expires_1)
                .await
                .unwrap()
        );

        // Past expiry is bad witnessing.
        let past = (now_0 - std::time::Duration::from_secs(1)).unwrap();
        let (nonce_2, _expires_2) = super::fresh_nonce(past).unwrap();

        assert_eq!(
            WitnessNonceResult::Expired,
            super::witness_nonce(&db, agent_0.clone(), nonce_2.clone(), past, past)
                .await
                .unwrap()
        );

        // Far future expiry is bad witnessing.
        let future = (Timestamp::now() + std::time::Duration::from_secs(1_000_000)).unwrap();
        let (nonce_3, expires_3) = super::fresh_nonce(future).unwrap();

        assert_eq!(
            WitnessNonceResult::Future,
            super::witness_nonce(&db, agent_0.clone(), nonce_3.clone(), now_1, expires_3)
                .await
                .unwrap()
        );

        // Expired nonce can be reused.
        let now_2 = Timestamp::now();
        let (nonce_4, expires_4) = super::fresh_nonce(now_2).unwrap();

        assert_eq!(
            WitnessNonceResult::Fresh,
            super::witness_nonce(&db, agent_0.clone(), nonce_4.clone(), now_2, expires_4)
                .await
                .unwrap()
        );
        assert_eq!(
            WitnessNonceResult::Duplicate,
            super::witness_nonce(&db, agent_0.clone(), nonce_4.clone(), now_2, expires_4)
                .await
                .unwrap()
        );
        let later = (expires_4 + std::time::Duration::from_millis(1)).unwrap();
        let (_nonce_5, later_expires) = super::fresh_nonce(later).unwrap();
        assert_eq!(
            WitnessNonceResult::Fresh,
            super::witness_nonce(&db, agent_0, nonce_4, later, later_expires)
                .await
                .unwrap()
        );
    }
}
