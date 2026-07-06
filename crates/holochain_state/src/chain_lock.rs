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
    /// Construct a `ChainLock` from its parts.
    ///
    /// Used by the [`DhtStore`](crate::dht_store::DhtStore) chain-lock wrapper to
    /// build a `ChainLock` from a `holochain_data` row. The returned lock is not
    /// filtered by expiry; callers decide whether it is still valid via
    /// [`ChainLock::is_expired_at`].
    pub(crate) fn from_parts(subject: Vec<u8>, expires_at: Timestamp) -> Self {
        Self {
            subject,
            expires_at,
        }
    }

    /// Get the subject of the lock.
    pub fn subject(&self) -> &[u8] {
        &self.subject
    }

    /// Check whether the lock is still valid at the given time.
    pub fn is_expired_at(&self, timestamp: Timestamp) -> bool {
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
