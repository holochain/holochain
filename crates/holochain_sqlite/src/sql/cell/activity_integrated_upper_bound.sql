SELECT
  author,
  MAX(seq)
FROM
  DhtOp
  JOIN Action ON DhtOp.action_hash = Action.hash
WHERE
  DhtOp.when_integrated IS NOT NULL
  AND DhtOp.type = :register_activity
GROUP BY
  Action.author
