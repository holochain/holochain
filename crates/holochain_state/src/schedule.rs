use crate::prelude::*;
use holochain_serialized_bytes::SerializedBytesError;
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
