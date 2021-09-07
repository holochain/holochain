use crate::prelude::StateMutationResult;
use holochain_sqlite::rusqlite::OptionalExtension;
use holochain_sqlite::rusqlite::{named_params, Transaction};

pub fn fn_is_scheduled(txn: &Transaction, scheduled_fn: String) -> StateMutationResult<bool> {
    match txn
        .query_row(
            "
            SELECT 1
            FROM ScheduledFunctions
            WHERE scheduled_fn == :scheduled_fn
            LIMIT 1
            ",
            named_params! {
                ":scheduled_fn": scheduled_fn
            },
            |row| row.get::<_, u32>(0),
        )
        .optional()?
    {
        Some(_) => Ok(true),
        None => Ok(false),
    }
}
