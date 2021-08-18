use crate::prelude::StateMutationResult;
use holochain_sqlite::rusqlite::OptionalExtension;
use holochain_sqlite::rusqlite::{named_params, Transaction};

/// True if the chain is currently locked for the given lock id.
/// The chain is never locked for the id that created it.
/// The chain is always locked for all other ids until the lock end time is in the past.
pub fn is_chain_locked(txn: &Transaction, lock: &[u8]) -> StateMutationResult<bool> {
    match txn
        .query_row(
            "
            SELECT 1
            FROM ChainLock
            WHERE end >= :now
            AND lock != :lock
            LIMIT 1
            ",
            named_params! {
                ":lock": lock,
                ":now": holochain_types::timestamp::now().0
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
            SELECT end
            FROM ChainLock
            WHERE
            lock = :lock
            ",
            named_params! {
                ":lock": lock,
            },
            |row| Ok(row.get::<_, i64>("end")? < holochain_types::timestamp::now().0),
        )
        .optional()?;
    // If there's no lock then it's expired.
    Ok(r.unwrap_or(true))
}
