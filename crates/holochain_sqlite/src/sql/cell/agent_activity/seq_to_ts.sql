SELECT
  DhtOp.authored_timestamp
FROM
  Action
  JOIN DhtOp ON DhtOp.action_hash = Action.hash
WHERE
  Action.seq = :seq
  AND DhtOp.type = :activity
  AND Action.author = :author