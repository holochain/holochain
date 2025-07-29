SELECT
  DhtOp.hash,
  DhtOp.basis_hash
FROM
  DhtOp
WHERE
  DhtOp.hash IN rarray(:hashes)
