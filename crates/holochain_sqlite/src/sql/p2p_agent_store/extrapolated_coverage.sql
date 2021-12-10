SELECT
  CASE WHEN (storage_start_loc <= storage_end_loc) THEN
    IFNULL(storage_end_loc - storage_start_loc, 0)
  ELSE
    IFNULL(4294967295 - storage_start_loc + storage_end_loc, 0)
  END AS dist
FROM
  p2p_agent_store
WHERE
  :now IS NOT NULL
  AND :start_loc IS NOT NULL
  AND :end_loc IS NOT NULL
--  is_active = TRUE
--  AND expires_at_ms >= :now
--  AND (
--    ( :start_loc <= :end_loc
--      AND storage_center_loc >= :start_loc
--      AND storage_center_loc <= :end_loc
--    ) OR ( :start_loc > :end_loc
--      AND ( storage_center_loc >= :start_loc
--        OR storage_center_loc <= :end_loc
--      )
--    )
--  );
;
