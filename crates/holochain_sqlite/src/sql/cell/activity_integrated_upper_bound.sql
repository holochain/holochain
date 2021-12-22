SELECT
  seq
FROM
  DhtOp
  JOIN Header ON DhtOp.header_hash = Header.hash
WHERE
  when_integrated IS NOT NULL
  AND DhtOp.type = :register_activity
ORDER BY
  seq DESC
LIMIT
  1
-- TODO this needs to be per agent.