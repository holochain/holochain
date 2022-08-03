-- simple select the matching agent
SELECT
  nonce
FROM
  nonce
WHERE
  agent = :agent;
