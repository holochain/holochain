-- select all only uses primary key, no need for an extra index
SELECT encoded
FROM p2p_store
WHERE space = :space AND agent = :agent
-- filter out any entries shadowed by a newer signed_at_ms
GROUP BY space, agent HAVING signed_at_ms = max(signed_at_ms)
;
