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
      DhtOp AS DhtOpDep
    WHERE
      DhtOpDep.action_hash = DhtOp.dependency
      AND DhtOpDep.when_integrated IS NOT NULL
      AND DhtOpDep.type = :store_record
    LIMIT
      1
  )
RETURNING
  hash,
  authored_timestamp
