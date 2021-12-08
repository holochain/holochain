DELETE FROM
  ScheduledFunctions
WHERE
  ephemeral = TRUE
  AND author = :author
