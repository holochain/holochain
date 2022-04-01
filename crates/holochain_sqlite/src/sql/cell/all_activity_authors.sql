SELECT
  author
FROM
  DhtOp
  JOIN Header ON DhtOp.header_hash = Header.hash
WHERE
  DhtOp.type = :register_activity
  AND DhtOp.validation_stage = 3
  AND DhtOp.validation_status IS NOT NULL
GROUP BY
  author