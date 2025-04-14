SELECT
  DhtOp.hash
FROM
  DhtOp
WHERE
  DhtOp.hash IN rarray(:hashes)
