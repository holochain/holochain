-- delete all expired entries from the p2p_store
DELETE
FROM p2p_store
WHERE expires_at_ms <= :now
;
