-- Use this query only in the continuous arc case,
-- i.e. when :storage_start_loc <= :storage_end_loc
SELECT
    hash, authored_timestamp_ms
FROM
  DHtOp
WHERE
  DhtOp.authored_timestamp_ms >= :from
  AND DhtOp.authored_timestamp_ms < :to
  AND storage_center_loc >= :storage_start_loc
  AND storage_center_loc <= :storage_end_loc
