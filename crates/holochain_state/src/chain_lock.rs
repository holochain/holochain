use crate::prelude::StateMutationResult;
use holo_hash::AgentPubKey;
use holochain_sqlite::rusqlite::OptionalExtension;
use holochain_sqlite::rusqlite::{named_params, Transaction};
use holochain_zome_types::prelude::*;

/// Represents a lock on an author's source chain.
///
/// The subject is used to identify the lock. If you took out the lock then you should know what is
/// in the subject. The expires_at field is the time at which the lock will expire.
///
/// Note that the lock is not automatically removed when it expires. It is up to the caller to
/// check the expiry timestamp to determine if the lock is still valid.
#[derive(Debug, Clone)]
pub struct ChainLock {
    subject: Vec<u8>,
    expires_at: Timestamp,
}

impl ChainLock {
    /// Get the subject of the lock.
    pub fn subject(&self) -> &[u8] {
        &self.subject
    }

    /// Check whether the lock is expired at the current time.
    pub fn is_expired(&self) -> bool {
        self.is_expired_at(Timestamp::now())
    }

    /// Check whether the lock is still valid at the given time.
    fn is_expired_at(&self, timestamp: Timestamp) -> bool {
        timestamp > self.expires_at
    }
}

/// Get the chain lock for the given author.
///
/// If the chain is locked, then a [ChainLock] is returned. Otherwise, `None` is returned.
pub fn get_chain_lock(
    txn: &Transaction,
    author: &AgentPubKey,
) -> StateMutationResult<Option<ChainLock>> {
    Ok(txn
        .query_row(
            "
            SELECT subject, expires_at_timestamp
            FROM ChainLock
            WHERE author = :author
            ",
            named_params! {
                ":author": author,
            },
            |row| {
                Ok(ChainLock {
                    subject: row.get(0)?,
                    expires_at: row.get(1)?,
                })
            },
        )
        .optional()?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::prelude::{lock_chain, unlock_chain, StateMutationError};
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
        let lock = db
            .read_async({
                let agent_pub_key = agent_pub_key.clone();
                move |txn| get_chain_lock(&txn, &agent_pub_key)
            })
            .await
            .unwrap();
        assert!(lock.is_none());

        // Lock the chain
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
        let lock = db
            .read_async({
                let agent_pub_key = agent_pub_key.clone();
                move |txn| get_chain_lock(&txn, &agent_pub_key)
            })
            .await
            .unwrap();
        assert!(lock.is_some());
        assert!(!lock.as_ref().unwrap().is_expired());
        assert_eq!(&[1, 2, 3], lock.as_ref().unwrap().subject());
        // In the future, the lock should be expired
        assert!(lock.unwrap().is_expired_at(Timestamp::now().add(Duration::from_secs(12)).unwrap()));

        // Now let's unlock the chain
        db.write_async({
            let agent_pub_key = agent_pub_key.clone();
            move |txn| unlock_chain(txn, &agent_pub_key)
        })
        .await
        .unwrap();

        // Which should make the chain unlocked
        let lock = db
            .read_async({
                let agent_pub_key = agent_pub_key.clone();
                move |txn| get_chain_lock(&txn, &agent_pub_key)
            })
            .await
            .unwrap();
        assert!(lock.is_none());
    }

    #[tokio::test]
    async fn cannot_hold_multiple_locks() {
        let agent_pub_key = AgentPubKey::from_raw_36(vec![0; 36]);
        let db = DbWrite::test_in_mem(DbKindAuthored(Arc::new(CellId::new(
            DnaHash::from_raw_36(vec![1; 36]),
            agent_pub_key.clone(),
        ))))
        .unwrap();

        // Create an initial lock
        db.write_async({
            let agent_pub_key = agent_pub_key.clone();
            move |txn| {
                let timestamp = Timestamp::now().add(Duration::from_secs(10)).unwrap();
                lock_chain(txn, &agent_pub_key, &[1, 2, 3], &timestamp)
            }
        })
        .await
        .unwrap();

        let check_is_constraint_err = |err: StateMutationError| match err {
            StateMutationError::Sql(e) => {
                assert_eq!(
                    holochain_sqlite::rusqlite::ErrorCode::ConstraintViolation,
                    e.sqlite_error_code().unwrap()
                );
            }
            _ => panic!("Expected a SQL error"),
        };

        // Try to create a second lock
        let err = db
            .write_async({
                let agent_pub_key = agent_pub_key.clone();
                move |txn| {
                    let timestamp = Timestamp::now().add(Duration::from_secs(10)).unwrap();
                    lock_chain(txn, &agent_pub_key, &[1, 2, 3], &timestamp)
                }
            })
            .await
            .unwrap_err();
        check_is_constraint_err(err);

        // Try to create a second lock with a different subject
        let err = db
            .write_async({
                let agent_pub_key = agent_pub_key.clone();
                move |txn| {
                    let timestamp = Timestamp::now().add(Duration::from_secs(10)).unwrap();
                    lock_chain(txn, &agent_pub_key, &[1, 2, 4], &timestamp)
                }
            })
            .await
            .unwrap_err();
        check_is_constraint_err(err);

        // Check that the chain is still locked
        let lock = db
            .read_async({
                let agent_pub_key = agent_pub_key.clone();
                move |txn| get_chain_lock(&txn, &agent_pub_key)
            })
            .await
            .unwrap();
        assert!(lock.is_some());
    }
}
