SELECT
  seq
FROM
  Action
  JOIN DhtOp ON DhtOp.action_hash = Action.hash
WHERE
  DhtOp.authored_timestamp = :authored_timestamp AND
  DhtOp.type = :activity
  AND Action.author = :author
