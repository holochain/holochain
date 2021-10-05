-- Use this query only in the continuous arc case,
-- i.e. when :storage_start_loc <= :storage_end_loc
SELECT
  hash,
  authored_timestamp
FROM
  DHtOp
WHERE
  DhtOp.authored_timestamp >= :from
  AND DhtOp.authored_timestamp < :to
  AND storage_center_loc >= :storage_start_loc
  AND storage_center_loc <= :storage_end_loc
