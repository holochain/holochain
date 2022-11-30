DELETE FROM
  ScheduledFunctions
WHERE
  ephemeral = TRUE
  AND START <= :now
  AND author = :author
