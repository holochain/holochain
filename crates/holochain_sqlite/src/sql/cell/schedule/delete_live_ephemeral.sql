DELETE FROM
  ScheduledFunctions
WHERE
  ephemeral = TRUE
  AND START <= ?
