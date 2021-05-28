SELECT MAX(timestamp)
FROM p2p_metrics
WHERE agent = :agent
AND   metric = :metric
