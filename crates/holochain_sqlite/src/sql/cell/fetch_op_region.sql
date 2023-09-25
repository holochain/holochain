SELECT
  COUNT(DhtOp.hash) AS count,
  REDUCE_XOR(DhtOp.hash) AS xor_hash,
  TOTAL(LENGTH(Action.blob)) AS total_action_size,
  -- We need to only account for entry data in the size count when the op contains the entry itself.
  -- Other ops refer to actions that refer to entries, but we don't want to include that in the size.
  TOTAL(
    CASE
      WHEN DhtOp.type IN ('StoreEntry', 'StoreRecord') THEN LENGTH(Entry.blob)
      ELSE 0
    END
  ) AS total_entry_size
FROM
  DhtOp
  JOIN Action ON DhtOp.action_hash = Action.hash
  LEFT JOIN Entry ON Action.entry_hash = Entry.hash
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
  )
