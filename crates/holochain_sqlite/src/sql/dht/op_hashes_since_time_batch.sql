SELECT
  hash,
  authored_timestamp,
  serialized_size,
  rowid
FROM
  DhtOp
WHERE
  (
    (
      -- non-wrapping case: everything within the given range
      :storage_start_loc <= :storage_end_loc
      AND (
        storage_center_loc >= :storage_start_loc
        AND storage_center_loc <= :storage_end_loc
      )
    )
    OR (
      -- wrapping case: everything *outside* the given range
      :storage_start_loc > :storage_end_loc
      AND (
        storage_center_loc <= :storage_end_loc
        OR storage_center_loc >= :storage_start_loc
      )
    )
  )
  -- op integrated is after the start time
  AND when_integrated >= :timestamp_min
ORDER BY
  when_integrated ASC
LIMIT
  :limit
