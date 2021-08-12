UPDATE
  p2p_agent_store
SET
  -- storage_center_loc = :storage_center_loc,
  storage_start_loc = :storage_start_loc,
  storage_end_loc = :storage_end_loc
WHERE
  agent = :agent
;
