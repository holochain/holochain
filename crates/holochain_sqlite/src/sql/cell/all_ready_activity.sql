SELECT
  seq
FROM
  DhtOp
  JOIN Action ON DhtOp.action_hash = Action.hash
WHERE
  DhtOp.type = :register_activity
  AND DhtOp.validation_stage = 3
  AND DhtOp.validation_status IS NOT NULL
  AND author = :author
ORDER BY
  seq ASC
