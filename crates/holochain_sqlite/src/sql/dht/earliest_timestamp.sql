SELECT
  MIN(authored_timestamp)
FROM
  DhtOp
WHERE
  (
    (
      -- non-wrapping case: everything within the given range
      :storage_start_loc <= :storage_end_loc
      AND storage_center_loc >= :storage_start_loc
      AND storage_center_loc <= :storage_end_loc
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
  -- ops are integrated, i.e. not in limbo
  AND when_integrated IS NOT NULL
