use crate::prelude::*;
use holo_hash::AgentPubKey;
use holochain_serialized_bytes::SerializedBytesError;
use holochain_sqlite::rusqlite::OptionalExtension;
use holochain_sqlite::rusqlite::{named_params, Transaction};
use std::str::FromStr;

/// Serialize an `Option<Schedule>` into the blob stored in the `maybe_schedule`
/// column of the `ScheduledFunction` table.
pub fn serialize_maybe_schedule(
    maybe_schedule: &Option<Schedule>,
) -> Result<Vec<u8>, SerializedBytesError> {
    holochain_serialized_bytes::encode(maybe_schedule)
}

/// Compute the `(start_at, end_at, ephemeral)` row values for a scheduled
/// function given its schedule and the current time.
///
/// Returns `Ok(None)` when the schedule is a `Persisted` cron string that has
/// no further dates after `now` — the caller should delete the function row
/// rather than inserting a new one.
pub fn compute_schedule_params(
    maybe_schedule: &Option<Schedule>,
    now: Timestamp,
) -> Result<Option<(Timestamp, Timestamp, bool)>, ScheduleError> {
    match maybe_schedule {
        Some(Schedule::Persisted(ref schedule_string)) => {
            let after =
                chrono::DateTime::<chrono::Utc>::try_from(now).map_err(ScheduleError::Timestamp)?;
            let start = match cron::Schedule::from_str(schedule_string)
                .map_err(|e| ScheduleError::Cron(e.to_string()))?
                .after(&after)
                .next()
            {
                Some(s) => s,
                // No further cron dates: caller should delete.
                None => return Ok(None),
            };
            let end = start
                + chrono::Duration::from_std(holochain_zome_types::schedule::PERSISTED_TIMEOUT)
                    .map_err(|e| ScheduleError::Cron(e.to_string()))?;
            Ok(Some((Timestamp::from(start), Timestamp::from(end), false)))
        }
        Some(Schedule::Ephemeral(duration)) => Ok(Some((
            (now + *duration).map_err(ScheduleError::Timestamp)?,
            Timestamp::max(),
            true,
        ))),
        None => Ok(Some((now, Timestamp::max(), true))),
    }
}

pub fn fn_is_scheduled(
    txn: &Transaction,
    scheduled_fn: ScheduledFn,
    author: &AgentPubKey,
) -> StateMutationResult<bool> {
    Ok(txn
        .query_row(
            "
            SELECT zome_name, scheduled_fn
            FROM ScheduledFunctions
            WHERE
            zome_name=:zome_name
            AND scheduled_fn=:scheduled_fn
            AND author = :author
            LIMIT 1
            ",
            named_params! {
                ":zome_name": scheduled_fn.zome_name().to_string(),
                ":scheduled_fn": scheduled_fn.fn_name().to_string(),
                ":author": author,
            },
            |row| row.get::<_, String>(0),
        )
        .optional()?
        .is_some())
}

/// Get a list of "live" scheduled functions.
///
/// A scheduled function is "live" if it is in the database with `now` between its start and end times.
///
/// Returns the list of scheduled functions with their next schedule and a bool indicating
/// if the schedule is ephemeral or not.
pub fn live_scheduled_fns(
    txn: &Transaction,
    now: Timestamp,
    author: &AgentPubKey,
) -> StateMutationResult<Vec<(ScheduledFn, Option<Schedule>, bool)>> {
    let mut stmt = txn.prepare(
        "
        SELECT
        zome_name,
        scheduled_fn,
        maybe_schedule,
        ephemeral
        FROM ScheduledFunctions
        WHERE
        start <= :now
        AND :now <= end
        AND author = :author
        ORDER BY start ASC",
    )?;
    let rows = stmt.query_map(
        named_params! {
            ":now": now,
            ":author": author,
        },
        |row| {
            Ok((
                ScheduledFn::new(
                    ZomeName(row.get::<_, String>(0)?.into()),
                    FunctionName(row.get(1)?),
                ),
                row.get(2)?,
                row.get(3)?,
            ))
        },
    )?;
    let mut ret = vec![];
    for row in rows {
        let (scheduled_fn, maybe_schedule_serialized, ephemeral) = row?;
        ret.push((
            scheduled_fn,
            from_blob(maybe_schedule_serialized)?,
            ephemeral,
        ));
    }
    Ok(ret)
}
