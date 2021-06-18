-- simple select the matching agent
SELECT encoded
FROM p2p_agent_store
WHERE agent = :agent
  AND is_active = TRUE
;
