SELECT
  seq,
  DhtOp.authored_timestamp
FROM
  Action
  JOIN DhtOp ON DhtOp.action_hash = Action.hash
WHERE
  DhtOp.authored_timestamp >= :from
  AND DhtOp.authored_timestamp <= :to
  AND DhtOp.type = :activity
  AND Action.author = :author
ORDER BY
    seq ASC
