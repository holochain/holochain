UPDATE
  DhtOp
SET
  when_integrated = :when_integrated,
  validation_stage = NULL
WHERE
  validation_stage = 3
  AND validation_status IS NOT NULL
  AND DhtOp.type = :register_activity
  AND DhtOp.header_hash IN (
    SELECT
      hash
    FROM
      Header
    WHERE
      seq > :activity_integrated
      AND seq < :activity_missing
      AND author = :author
  )