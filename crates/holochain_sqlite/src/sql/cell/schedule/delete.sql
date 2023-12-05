DELETE FROM
  ScheduledFunctions
WHERE
  zome_name = :zome_name
  AND scheduled_fn = :scheduled_fn
  AND author = :author
