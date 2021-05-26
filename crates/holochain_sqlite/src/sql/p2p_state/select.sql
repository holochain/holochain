-- simple select the matching agent
SELECT encoded
FROM p2p_store
WHERE agent = :agent
;
