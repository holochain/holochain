INSERT
  OR IGNORE INTO p2p_metrics (
    kind,
    agent,
    recorded_at_utc_micros,
    expires_at_utc_micros,
    data
  )
VALUES
  (
    :kind,
    :agent,
    :recorded_at_utc_micros,
    :expires_at_utc_micros,
    :data
  );
