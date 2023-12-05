DELETE FROM
  p2p_metrics
WHERE
  expires_at_utc_micros <= :now_micros;
