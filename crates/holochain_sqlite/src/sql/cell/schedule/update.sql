UPDATE
  ScheduledFunctions
SET
  maybe_schedule = :maybe_schedule,
  START = :start,
END = :end,
ephemeral = :ephemeral
WHERE
  zome_name = :zome_name
  AND scheduled_fn = :scheduled_fn
  AND author = :author
