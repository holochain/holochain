SELECT SUM(LENGTH(Action.blob))
FROM Action,
    DhtOp
WHERE DhtOp.authored_timestamp > ?1
    AND DhtOp.action_hash = Action.hash;