SELECT
  o.agent,
  MIN(o.moment)
FROM
  (
    SELECT
      agent,
      MAX(moment) AS most_recent_error
    FROM
      p2p_metrics
    WHERE
      kind = :kind_error
    GROUP BY
      agent
  ) AS i
  JOIN p2p_metrics o ON i.agent = o.agent
WHERE
  i.most_recent_error < :error_threshold
  AND kind = :kind_slow_gossip
