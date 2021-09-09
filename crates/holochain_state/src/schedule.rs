use crate::prelude::StateMutationResult;
use holochain_sqlite::rusqlite::OptionalExtension;
use holochain_sqlite::rusqlite::{named_params, Transaction};

pub fn fn_is_scheduled(txn: &Transaction, zome_name: String, scheduled_fn: String) -> StateMutationResult<bool> {
    match txn
        .query_row(
            "
            SELECT 1
            FROM ScheduledFunctions
            WHERE
            zome_name = :zome_name
            AND scheduled_fn = :scheduled_fn
            LIMIT 1
            ",
            named_params! {
                ":zome_name": zome_name,
                ":scheduled_fn": scheduled_fn,
            },
            |row| row.get::<_, u32>(0),
        )
        .optional()?
    {
        Some(_) => Ok(true),
        None => Ok(false),
    }
}

pub fn live_scheduled_fns(txn: &Transaction, now: Timestamp) -> StateMutationResult<Vec<(zome_name, scheduled_fn, schedule)>> {
    txn.execute(
        "
        SELECT
        zome_name,
        scheduled_fn,
        schedule
        FROM ScheduledFunctions
        WHERE
        start <= :now
        AND :now <= end",
        named_params! {
            ":now": now
        },
    )?.query_map(|row| {
        Ok((row.get(0)?, row.get(1)?, row.get(2)?))
    })
}