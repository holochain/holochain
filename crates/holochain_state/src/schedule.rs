use crate::prelude::StateMutationResult;
use crate::query::from_blob;
use holochain_sqlite::rusqlite::OptionalExtension;
use holochain_sqlite::rusqlite::{named_params, Transaction};
use holochain_zome_types::FunctionName;
use holochain_zome_types::Schedule;
use holochain_zome_types::ScheduledFn;
use holochain_zome_types::Timestamp;
use holochain_zome_types::ZomeName;

pub fn fn_is_scheduled(txn: &Transaction, scheduled_fn: ScheduledFn) -> StateMutationResult<bool> {
    match txn
        .query_row(
            "
            SELECT zome_name, scheduled_fn
            FROM ScheduledFunctions
            WHERE
            zome_name=:zome_name
            AND scheduled_fn=:scheduled_fn
            LIMIT 1
            ",
            named_params! {
                ":zome_name": scheduled_fn.zome_name().to_string(),
                ":scheduled_fn": scheduled_fn.fn_name().to_string(),
            },
            |row| row.get::<_, String>(0),
        )
        .optional()?
    {
        Some(_) => {
            dbg!("fn_is_scheduled true", &scheduled_fn);
            Ok(true)
        },
        None => {
            dbg!("fn_is_scheduled false", &scheduled_fn);
            Ok(false)
        },
    }
}

pub fn live_scheduled_fns(
    txn: &Transaction,
    now: Timestamp,
) -> StateMutationResult<Vec<(ScheduledFn, Option<Schedule>)>> {
    let mut stmt = txn.prepare(
        "
        SELECT
        zome_name,
        scheduled_fn,
        maybe_schedule
        FROM ScheduledFunctions
        WHERE
        start <= ?
        AND ? <= end",
    )?;
    let rows = stmt.query_map([now.to_sql_ms_lossy(), now.to_sql_ms_lossy()], |row| {
        Ok((
            ScheduledFn::new(ZomeName(row.get(0)?), FunctionName(row.get(1)?)),
            row.get(2)?,
        ))
    })?;
    let mut ret = vec![];
    for row in rows {
        let (scheduled_fn, maybe_schedule_serialized) = row?;
        ret.push((scheduled_fn, from_blob(maybe_schedule_serialized)?));
    }
    dbg!(&ret);
    Ok(ret)
}
