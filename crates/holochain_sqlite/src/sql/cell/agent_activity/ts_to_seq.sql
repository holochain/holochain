SELECT
  seq
FROM
  Action
  JOIN DhtOp ON DhtOp.action_hash = Action.hash
WHERE
  DhtOp.authored_timestamp >= :until
  AND DhtOp.type = :activity
  AND Action.author = :author
ORDER BY
  seq ASC
