UPDATE
  DhtOp
SET
  when_integrated = :when_integrated,
  validation_stage = NULL
WHERE
  validation_stage = 3
  AND validation_status IS NOT NULL
  AND DhtOp.type = :register_activity
  AND DhtOp.action_hash IN (
    SELECT
      hash
    FROM
      'Action'
    WHERE
      seq >= :seq_start
      AND seq <= :seq_end
      AND author = :author
  )
