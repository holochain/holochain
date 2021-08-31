SELECT
  hash
FROM
  DHtOp
WHERE
  DhtOp.authored_timestamp_ms >= :from
  AND DhtOp.authored_timestamp_ms < :to
  AND storage_center_loc >= :storage_start_1
  AND storage_center_loc <= :storage_end_1
  AND DhtOp.when_integrated IS NOT NULL
  AND private_entry IS NULL
