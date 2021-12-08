use crate::prelude::StateMutationResult;
use holo_hash::AgentPubKey;
use holochain_sqlite::rusqlite::OptionalExtension;
use holochain_sqlite::rusqlite::{named_params, Transaction};
use holochain_zome_types::Timestamp;

/// True if the chain is currently locked for the given lock id.
/// The chain is never locked for the id that created it.
/// The chain is always locked for all other ids until the lock end time is in the past.
pub fn is_chain_locked(
    txn: &Transaction,
    lock: &[u8],
    author: &AgentPubKey,
) -> StateMutationResult<bool> {
    let mut lock = lock.to_vec();
    lock.extend(author.get_raw_39());
    match txn
        .query_row(
            "
            SELECT 1
            FROM ChainLock
            WHERE expires_at_timestamp >= :now
            AND lock != :lock
            AND author = :author
            LIMIT 1
            ",
            named_params! {
                ":lock": lock,
                ":author": author,
                ":now": holochain_zome_types::Timestamp::now()
            },
            |row| row.get::<_, u32>(0),
        )
        .optional()?
    {
        Some(_) => Ok(true),
        None => Ok(false),
    }
}

/// Check if a lock is expired.
pub fn is_lock_expired(
    txn: &Transaction,
    lock: &[u8],
    author: &AgentPubKey,
) -> StateMutationResult<bool> {
    let mut lock = lock.to_vec();
    lock.extend(author.get_raw_39());
    let r = txn
        .query_row(
            "
            SELECT expires_at_timestamp
            FROM ChainLock
            WHERE
            lock = :lock
            ",
            named_params! {
                ":lock": lock,
            },
            |row| {
                Ok(row.get::<_, Timestamp>("expires_at_timestamp")?
                    < holochain_zome_types::Timestamp::now())
            },
        )
        .optional()?;
    // If there's no lock then it's expired.
    Ok(r.unwrap_or(true))
}
