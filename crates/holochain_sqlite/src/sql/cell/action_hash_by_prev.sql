SELECT
  hash,
  blob
FROM
  Action
WHERE
  prev_hash = :prev_hash
LIMIT
  1
