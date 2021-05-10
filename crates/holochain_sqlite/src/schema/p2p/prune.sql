-- delete all from our two unioned sub queries
DELETE
FROM p2p_store
WHERE rowid IN (
  -- first pick out all the items shadowed by a newer signed_at_ms
  SELECT rowid
  FROM p2p_store
  WHERE rowid NOT IN (
    SELECT rowid
    FROM p2p_store
    GROUP BY space, agent HAVING signed_at_ms = max(signed_at_ms)
  )
  UNION ALL
  -- next pick out all items that are expired
  SELECT rowid
  FROM p2p_store
  WHERE expires_at_ms <= :expires_at_ms
);
