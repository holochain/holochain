use crate::prelude::StateMutationResult;
use holochain_sqlite::rusqlite::OptionalExtension;
use holochain_sqlite::rusqlite::{named_params, Transaction};
use holochain_zome_types::Timestamp;

/// True if the chain is currently locked for the given lock id.
/// The chain is never locked for the id that created it.
/// The chain is always locked for all other ids until the lock end time is in the past.
pub fn is_chain_locked(txn: &Transaction, lock: &[u8]) -> StateMutationResult<bool> {
    match txn
        .query_row(
            "
            SELECT 1
            FROM ChainLock
            WHERE expires_at_timestamp >= :now
            AND lock != :lock
            LIMIT 1
            ",
            named_params! {
                ":lock": lock,
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
pub fn is_lock_expired(txn: &Transaction, lock: &[u8]) -> StateMutationResult<bool> {
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
