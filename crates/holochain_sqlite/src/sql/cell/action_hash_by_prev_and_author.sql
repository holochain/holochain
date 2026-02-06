SELECT
  hash,
  blob
FROM
  Action
WHERE
  hash != :hash
  AND prev_hash = :prev_hash
  AND author = :author
LIMIT
  1
