SELECT
  seq
FROM
  ACTION
  JOIN DhtOp ON DhtOp.action_hash = ACTION.hash
WHERE
  ACTION.hash = :hash
  AND DhtOp.type = :activity
  AND ACTION.author = :author