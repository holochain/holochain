SELECT
  DhtOp.hash,
  DhtOp.type,
  Header.blob AS header_blob,
  Entry.blob AS entry_blob
FROM
  DhtOp
  JOIN Header ON DhtOp.header_hash = Header.hash
  LEFT JOIN Entry ON Header.entry_hash = Entry.hash
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
  ) -- op timestamp is within temporal bounds
  AND (
    authored_timestamp >= :timestamp_min
    AND authored_timestamp <= :timestamp_max
  ) -- ops are integrated, i.e. not in limbo
  AND DhtOp.when_integrated IS NOT NULL
