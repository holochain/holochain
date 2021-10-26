SELECT
  hash
FROM
  DHtOp
WHERE
  DhtOp.authored_timestamp >= :from
  AND DhtOp.authored_timestamp < :to
  AND storage_center_loc >= :storage_start_1
  AND storage_center_loc <= :storage_end_1
ORDER BY
  authored_timestamp ASC
LIMIT
  :limit
