UPDATE
  DhtOp
SET
  when_integrated = :when_integrated,
  validation_stage = NULL
WHERE
  validation_stage = 3
  AND validation_status IS NOT NULL
  AND DhtOp.type = :register_activity
  AND EXISTS(
    SELECT
      1
    FROM
      Header
    WHERE
      DhtOp.header_hash = Header.hash
      AND seq > :activity_integrated
      AND seq < :activity_missing
    LIMIT
      1
  )
-- TODO this needs to be per agent.