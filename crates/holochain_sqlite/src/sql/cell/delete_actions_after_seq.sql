DELETE FROM
  Action
WHERE
  author = :author
  AND seq > :seq
