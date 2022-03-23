-- it's hard to construct queries with input arrays
-- we're taking the strategy here of concatenating a bunch of
-- local agent ids together, and extracting them here
WITH RECURSIVE split(src, cur_idx, slice) AS (
  -- we have to manually configure the first seed row
  SELECT
    :agent_list,
    37,
    substr(:agent_list, 1, 36)
  UNION
  ALL
  SELECT
    src,
    cur_idx + 36,
    substr(src, cur_idx, 36)
  FROM
    split
  WHERE
    cur_idx < length(src)
) -- delete all expired entries from the p2p_agent_store
DELETE FROM
  p2p_agent_store
WHERE
  expires_at_ms <= :now
  AND agent NOT IN (
    SELECT
      slice AS agent
    FROM
      split
  );
