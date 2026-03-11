SELECT
  COUNT(*)
FROM
  DhtOp
WHERE
  -- ops are integrated, i.e. not in limbo
  when_integrated IS NOT NULL
