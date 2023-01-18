-- simple select the matching agent
DELETE FROM
  p2p_agent_store
WHERE
  agent = :agent;
