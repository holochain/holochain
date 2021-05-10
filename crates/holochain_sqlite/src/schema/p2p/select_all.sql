-- select all only uses primary key, no need for an extra index
SELECT
  space,
  agent,
  signed_at_ms,
  expires_at_ms,
  encoded,
  storage_center_loc,
  storage_half_length,
  storage_start_1,
  storage_end_1,
  storage_start_2,
  storage_end_2
FROM p2p_store
WHERE space = :space
-- filter out any entries shadowed by a newer signed_at_ms
GROUP BY space, agent HAVING signed_at_ms = max(signed_at_ms)
;
