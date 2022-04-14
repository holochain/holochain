SELECT
  COUNT() AS count,
  TOTAL(LENGTH(Header.blob)) + TOTAL(LENGTH(Entry.blob)) AS total_size,
  REDUCE_XOR(DhtOp.hash) AS xor_hash
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
    AND authored_timestamp < :timestamp_max
  )
