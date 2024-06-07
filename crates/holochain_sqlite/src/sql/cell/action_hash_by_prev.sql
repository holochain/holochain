SELECT
  hash,
  blob
FROM
  Action
WHERE
  prev_hash = :prev_hash
  AND author = :author
LIMIT
  1
