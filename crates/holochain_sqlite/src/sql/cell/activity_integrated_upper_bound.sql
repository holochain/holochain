SELECT
  author,
  MAX(seq)
FROM
  DhtOp
  JOIN Header ON DhtOp.header_hash = Header.hash
WHERE
  DhtOp.when_integrated IS NOT NULL
  AND DhtOp.type = :register_activity
GROUP BY
  Header.author