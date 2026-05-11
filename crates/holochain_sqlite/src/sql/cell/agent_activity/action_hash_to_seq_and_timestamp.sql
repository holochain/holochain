SELECT
  Action.seq AS seq,
  DhtOp.authored_timestamp AS timestamp
FROM
  Action
  JOIN DhtOp ON DhtOp.action_hash = Action.hash
WHERE
  Action.hash = :action_hash
  AND Action.author = :author
  AND DhtOp.type = :op_type_register_agent_activity
  AND DhtOp.when_integrated IS NOT NULL