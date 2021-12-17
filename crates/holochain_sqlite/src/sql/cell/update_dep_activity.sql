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
      DhtOp AS OP_DEP
    WHERE
      OP_DEP.header_hash = DhtOp.dependency
      AND (
        OP_DEP.when_integrated IS NOT NULL
        OR OP_DEP.validation_stage = 3
      )
      AND validation_status IS NOT NULL
      AND OP_DEP.type = :register_activity
    LIMIT
      1
  )