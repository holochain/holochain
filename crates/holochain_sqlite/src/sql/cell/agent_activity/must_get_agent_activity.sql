SELECT
  ACTION.hash,
  ACTION.blob
FROM
  DhtOp
  JOIN ACTION ON DhtOp.action_hash = ACTION.hash
WHERE
  DhtOp.type = :op_type
  AND DhtOp.when_integrated IS NOT NULL
  AND ACTION.author = :author
  AND ACTION.seq BETWEEN :lower_seq
  AND :upper_seq