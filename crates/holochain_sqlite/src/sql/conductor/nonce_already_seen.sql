-- simple select the matching agent
SELECT
  1
FROM
  nonce
WHERE
  agent = :agent
  AND nonce = :nonce
  AND expires > :now