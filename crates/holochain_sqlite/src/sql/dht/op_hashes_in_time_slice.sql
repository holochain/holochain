SELECT
  hash,
  basis_hash,
  serialized_size
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
  -- op timestamp is within temporal bounds
  AND authored_timestamp >= :timestamp_min
  AND authored_timestamp < :timestamp_max
  -- ops are integrated, i.e. not in limbo
  AND when_integrated IS NOT NULL
ORDER BY
  authored_timestamp ASC
