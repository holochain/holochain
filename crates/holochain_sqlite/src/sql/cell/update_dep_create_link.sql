UPDATE
  DhtOp
SET
  when_integrated = :when_integrated,
  validation_stage = NULL
WHERE
  validation_stage = 3
  AND validation_status IS NOT NULL
  AND DhtOp.type = :delete_link
  AND EXISTS(
    SELECT
      1
    FROM
      DhtOp AS OP_DEP
    WHERE
      OP_DEP.action_hash = DhtOp.dependency
      AND OP_DEP.when_integrated IS NOT NULL
      AND OP_DEP.type = :create_link
    LIMIT
      1
  )
