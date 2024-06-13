SELECT
  hash,
  blob
FROM
  Action
WHERE
  hash != :hash
  AND prev_hash = :prev_hash
LIMIT
  1
