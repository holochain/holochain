SELECT agent
FROM p2p_agent_store
WHERE is_active = TRUE
  AND signed_at_ms >= :since_ms -- between given signed_at range
  AND signed_at_ms <= :until_ms
  AND (
    -- if the input has two ranges, check them both
    (
      :storage_start_1 IS NOT NULL
      AND :storage_end_1 IS NOT NULL
      AND :storage_start_2 IS NOT NULL
      AND :storage_end_2 IS NOT NULL
      AND (
        (
          storage_center_loc >= :storage_start_1
          AND storage_center_loc <= :storage_end_1
        )
        OR
        (
          storage_center_loc >= :storage_start_2
          AND storage_center_loc <= :storage_end_2
        )
      )
    )
    OR
    -- if the input has only one range, check it
    (
      :storage_start_1 IS NOT NULL
      AND :storage_end_1 IS NOT NULL
      AND storage_center_loc >= :storage_start_1
      AND storage_center_loc <= :storage_end_1
    )
    -- if the input has no ranges, no records will match
  )
;
