SELECT
  COUNT(DISTINCT ACTION.seq) AS unique_seq
FROM
  DhtOp
  JOIN ACTION ON DhtOp.action_hash = ACTION.hash
WHERE
  DhtOp.type = :op_type
  AND ACTION.author = :author
  AND ACTION.seq BETWEEN :lower_seq
  AND :upper_seq