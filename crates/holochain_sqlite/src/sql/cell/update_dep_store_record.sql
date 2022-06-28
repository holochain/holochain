UPDATE
  DhtOp
SET
  when_integrated = :when_integrated,
  validation_stage = NULL
WHERE
  validation_stage = 3
  AND validation_status IS NOT NULL
  AND DhtOp.type IN (:updated_record, :deleted_by)
  AND EXISTS(
    SELECT
      1
    FROM
      DhtOp AS OP_DEP
    WHERE
      OP_DEP.action_hash = DhtOp.dependency
      AND OP_DEP.when_integrated IS NOT NULL
      AND OP_DEP.type = :store_record
    LIMIT
      1
  )
