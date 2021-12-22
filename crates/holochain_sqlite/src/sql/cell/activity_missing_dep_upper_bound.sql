SELECT
  seq
FROM
  DhtOp AS a
  JOIN header ON a.header_hash = header.hash
WHERE
  a.type = :register_activity
  AND a.validation_stage = 3
  AND a.validation_status IS NOT NULL
  AND NOT EXISTS(
    SELECT
      1
    FROM
      DhtOp AS b
    WHERE
      a.header_hash = b.dependency
      AND b.type = :register_activity
      AND b.validation_stage = 3
      AND b.validation_status IS NOT NULL
    LIMIT
      1
  )
ORDER BY
  seq ASC
LIMIT
  1
-- TODO this needs to be per agent.