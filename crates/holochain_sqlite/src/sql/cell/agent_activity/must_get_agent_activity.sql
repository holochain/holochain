SELECT
  Action.hash,
  Action.blob
FROM
  DhtOp
  JOIN Action ON DhtOp.action_hash = Action.hash
WHERE
  DhtOp.type = :op_type
  AND DhtOp.when_integrated IS NOT NULL
  AND Action.author = :author
  AND Action.seq BETWEEN :lower_seq AND :upper_seq
