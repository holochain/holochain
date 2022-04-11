SELECT
  COUNT() AS count,
  SUM(LENGTH(Header.blob) + LENGTH(Entry.blob)) AS blobsize,
  REDUCE_XOR(hash) AS xor_hash
FROM
  DhtOp
  JOIN Header ON DhtOp.header_hash = Header.hash
  LEFT JOIN Entry ON Header.entry_hash = Entry.hash
WHERE
  author = :author -- op location is within location bounds
  AND (
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
