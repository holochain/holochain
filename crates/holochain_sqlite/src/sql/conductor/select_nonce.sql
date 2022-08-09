-- simple select the matching agent
SELECT
  nonce
FROM
  nonce
WHERE
  agent = :agent
  AND nonce = :nonce
  AND expires > :now