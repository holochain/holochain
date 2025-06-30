SELECT
  seq
FROM
  Action
  JOIN DhtOp ON DhtOp.action_hash = Action.hash
WHERE
  Action.hash = :hash
  AND DhtOp.type = :activity
  AND Action.author = :author
