-- because UPSERT isn't guaranteed to exist on our sqlite version
-- we need to fashion our own with an INSERT SELECT statement
INSERT INTO p2p_agent_store
SELECT
  :agent AS agent,
  :encoded AS encoded,
  :signed_at_ms AS signed_at_ms,
  :expires_at_ms AS expires_at_ms,
  :storage_center_loc AS storage_center_loc,
  :is_active AS is_active,
  :storage_start_1 AS storage_start_1,
  :storage_end_1 AS storage_end_1,
  :storage_start_2 AS storage_start_2,
  :storage_end_2 AS storage_end_2
WHERE (
  -- count the rows that should supercede the one we're trying to insert
  SELECT count(rowid)
  FROM p2p_agent_store
  WHERE agent = :agent
    AND signed_at_ms > :signed_at_ms
) = 0 -- if there are none, proceed with the insert
;
