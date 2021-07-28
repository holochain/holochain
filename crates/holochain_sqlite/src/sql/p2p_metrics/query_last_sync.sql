SELECT
  MAX(moment)
FROM
  p2p_metrics
WHERE
  agent = :agent
  AND kind = :kind
