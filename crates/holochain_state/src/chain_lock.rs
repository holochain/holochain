use crate::prelude::StateMutationResult;
use holo_hash::AgentPubKey;
use holochain_sqlite::rusqlite::OptionalExtension;
use holochain_sqlite::rusqlite::{named_params, Transaction};
use holochain_zome_types::prelude::*;

/// Checks whether the author's chain is locked.
///
/// Note that this takes expiry into account, so if the lock has expired then this function will
/// return false.
pub fn is_chain_locked(txn: &Transaction, author: &AgentPubKey) -> StateMutationResult<bool> {
    Ok(txn.query_row(
        "
            SELECT EXISTS(
                SELECT author
                FROM ChainLock
                WHERE author = :author
                AND expires_at_timestamp >= :now
            )
            ",
        named_params! {
            ":author": author,
            ":now": Timestamp::now()
        },
        |row| row.get::<_, bool>(0),
    )?)
}

/// Get the subject of the chain lock.
///
/// If the chain is not locked or the lock has expired then this function will return `None`.
/// Otherwise, it will return the subject of the lock that was specified when the chain was locked.
pub fn get_chain_lock_subject(
    txn: &Transaction,
    author: &AgentPubKey,
) -> StateMutationResult<Option<Vec<u8>>> {
    Ok(txn
        .query_row(
            "
            SELECT subject
            FROM ChainLock
            WHERE author = :author
            AND expires_at_timestamp >= :now
            ",
            named_params! {
                ":author": author,
                ":now": Timestamp::now()
            },
            |row| row.get(0),
        )
        .optional()?)
}

/// Check if the chain lock is expired.
///
/// If there is no lock then this function returns true. So it is important to check that the chain
/// is locked in the same transaction where you use this function, if you need to be able to
/// distinguish between the chain being unlocked and the lock being expired.
pub fn is_chain_lock_expired(txn: &Transaction, author: &AgentPubKey) -> StateMutationResult<bool> {
    is_chain_lock_expired_inner(txn, author, Timestamp::now())
}

#[inline]
fn is_chain_lock_expired_inner(
    txn: &Transaction,
    author: &AgentPubKey,
    at_time: Timestamp,
) -> StateMutationResult<bool> {
    let r = txn
        .query_row(
            "
            SELECT expires_at_timestamp < :now AS expired
            FROM ChainLock
            WHERE
            author = :author
            ",
            named_params! {
                ":author": author,
                ":now": at_time
            },
            |row| row.get::<_, bool>("expired"),
        )
        .optional()?;

    Ok(r.unwrap_or(true))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::prelude::{lock_chain, unlock_chain};
    use holochain_sqlite::db::{DbKindAuthored, DbWrite};
    use std::ops::Add;
    use std::sync::Arc;
    use std::time::Duration;

    #[tokio::test]
    async fn source_chain_lock() {
        let agent_pub_key = AgentPubKey::from_raw_36(vec![0; 36]);
        let db = DbWrite::test_in_mem(DbKindAuthored(Arc::new(CellId::new(
            DnaHash::from_raw_36(vec![1; 36]),
            agent_pub_key.clone(),
        ))))
        .unwrap();

        // The chain should not be locked initially
        let initially_locked = db
            .read_async({
                let agent_pub_key = agent_pub_key.clone();
                move |txn| is_chain_locked(&txn, &agent_pub_key)
            })
            .await
            .unwrap();
        assert!(!initially_locked);

        db.write_async({
            let agent_pub_key = agent_pub_key.clone();
            move |txn| {
                let timestamp = Timestamp::now().add(Duration::from_secs(10)).unwrap();
                lock_chain(txn, &agent_pub_key, &[1, 2, 3], &timestamp)
            }
        })
        .await
        .unwrap();

        // The chain should be locked now
        let locked = db
            .read_async({
                let agent_pub_key = agent_pub_key.clone();
                move |txn| is_chain_locked(&txn, &agent_pub_key)
            })
            .await
            .unwrap();
        assert!(locked);

        // The lock should not be expired
        let expired = db
            .read_async({
                let agent_pub_key = agent_pub_key.clone();
                move |txn| is_chain_lock_expired_inner(&txn, &agent_pub_key, Timestamp::now())
            })
            .await
            .unwrap();
        assert!(!expired);

        // We should be able to retrieve the subject of the lock
        let subject = db
            .read_async({
                let agent_pub_key = agent_pub_key.clone();
                move |txn| get_chain_lock_subject(&txn, &agent_pub_key)
            })
            .await
            .unwrap();
        assert_eq!(subject, Some(vec![1, 2, 3]));

        // In the future, the lock should be expired
        let expired = db
            .read_async({
                let agent_pub_key = agent_pub_key.clone();
                move |txn| {
                    let timestamp = Timestamp::now().add(Duration::from_secs(30)).unwrap();
                    is_chain_lock_expired_inner(&txn, &agent_pub_key, timestamp)
                }
            })
            .await
            .unwrap();
        assert!(expired);

        // Now let's unlock the chain
        db.write_async({
            let agent_pub_key = agent_pub_key.clone();
            move |txn| {
                let timestamp = Timestamp::now().add(Duration::from_secs(10)).unwrap();
                unlock_chain(txn, &agent_pub_key)
            }
        })
        .await
        .unwrap();

        // Which should make the chain unlocked
        let locked = db
            .read_async({
                let agent_pub_key = agent_pub_key.clone();
                move |txn| is_chain_locked(&txn, &agent_pub_key)
            })
            .await
            .unwrap();
        assert!(!locked);

        // Slightly strangely, the chain lock will be expired, even though there isn't one
        let expired = db
            .read_async({
                let agent_pub_key = agent_pub_key.clone();
                move |txn| is_chain_lock_expired_inner(&txn, &agent_pub_key, Timestamp::now())
            })
            .await
            .unwrap();
        assert!(expired);

        // And we shouldn't be able to retrieve the subject of the lock
        let subject = db
            .read_async({
                let agent_pub_key = agent_pub_key.clone();
                move |txn| get_chain_lock_subject(&txn, &agent_pub_key)
            })
            .await
            .unwrap();
        assert!(subject.is_none());
    }
}
