SELECT
  hash,
  authored_timestamp_ms
FROM
  DHtOp
WHERE
  is_authored = 1
  AND DhtOp.authored_timestamp_ms >= :from
  AND DhtOp.authored_timestamp_ms < :to
  AND private_entry IS NULL
